//! **INDEXING AND POINTERS**: turning `a[i]`, `*p` and `p + n` into an address.
//!
//! === Why this is a file of its own ===
//!
//! Because the five spellings C offers for reaching an element --`a[i]`,
//! `*(a+i)`, `p[i]`, `*(p+i)`, `&a[i]`-- **are the same sum**: a base, an index
//! and a STRIDE. All that changes is where each of the three comes from.
//!
//! Scattered through `emit_expr`, each spelling was an arm working out the
//! stride for itself. Together, the stride comes from one place
//! (`pointer_scale`) and the rule can be read.
//!
//! === ** What keeping them apart cost, three times over ===
//!
//! The STRIDE is the number that has failed more often than anything else in
//! this compiler:
//!
//! | | |
//! |---|---|
//! | `p + 1` on a `struct T *` | advanced ONE byte |
//! | `p++` on any pointer | advanced ONE byte |
//! | `&c->defaults[i]` | evaluated to ZERO |
//!
//! The three are one sum asked from three different places, and the three were
//! fixed separately on the same day. That is the argument for keeping them
//! together: **the third time you pay for the same bug, the fix is not the
//! missing case -- it is the layout.**

use super::*;

impl Codegen {
    /// `name` es un array (su memoria vive en el slot) o un puntero (el slot
    /// guarda una direccion)? La distincion que antes no existia y corrompia.
    pub(super) fn var_is_array(&self, name: &str) -> bool {
        if let Some(&(_, ref t)) = self.var_offsets.get(name) { return matches!(t, TypeSpec::Array(_, _)); }
        if let Some(&(_, ref t)) = self.global_offsets.get(name) { return matches!(t, TypeSpec::Array(_, _)); }
        false
    }

    /// Tipo del elemento de un array/puntero (para cargas/stores del tamano exacto).
    pub(super) fn elem_type_of(&self, name: &str) -> TypeSpec {
        let t = self.var_offsets.get(name).map(|&(_, ref t)| t.clone())
            .or_else(|| self.global_offsets.get(name).map(|&(_, ref t)| t.clone()));
        match t {
            Some(TypeSpec::Array(e, _)) | Some(TypeSpec::Ptr(e)) => *e,
            _ => TypeSpec::Long,
        }
    }

    /// rax = rax * scale (shl si es potencia de 2; imul si no -- structs)
    /// * Escalar el indice por el tamano de UN paso.
    ///
    /// El paso ya no cabe siempre en un byte: en `int grid[2][3]` un paso del
    /// indice de fuera es una FILA entera --doce bytes--, y en
    /// `gammatable[5][256]` son 256. Por eso hay tres formas y no dos, y la
    /// tercera es la que faltaba: `imul` con inmediato de 32 bits.
    pub(super) fn emit_scale_index(&mut self, scale: u32) {
        if scale <= 1 {
            return;
        }
        if scale.is_power_of_two() {
            // shl rax, log2(scale)
            self.code.extend_from_slice(&[0x48, 0xC1, 0xE0, scale.trailing_zeros() as u8]);
        } else if scale <= i8::MAX as u32 {
            // imul rax, rax, imm8
            self.code.extend_from_slice(&[0x48, 0x6B, 0xC0, scale as u8]);
        } else {
            // imul rax, rax, imm32
            self.code.extend_from_slice(&[0x48, 0x69, 0xC0]);
            self.code.extend_from_slice(&scale.to_le_bytes());
        }
    }

    /// rax = direccion de name[idx]. Array -> base = lea del slot;
    /// puntero -> base = VALOR del slot. Local o global.
    pub(super) fn emit_subscript_addr(&mut self, name: &str, index: &Expr, scale: u32) {
        self.emit_expr(index);
        self.emit_scale_index(scale);
        self.code.push(0x50); // push indice escalado
        if self.var_is_array(name) {
            if let Some(&(off, _)) = self.var_offsets.get(name) {
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x45, off as u8]); // lea rax,[rbp+off]
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                    self.code.extend_from_slice(&off.to_le_bytes());
                }
            } else {
                self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]); // lea rax,[rip+global]
                self.global_fixups.push((self.code.len() - 4, name.to_string()));
            }
        } else {
            self.emit_load_var(name); // rax = valor del puntero
        }
        self.code.push(0x5A); // pop rdx = indice escalado
        self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
    }

    /// rax += offset (encoding corto si cabe en imm8)
    pub(super) fn emit_add_offset(&mut self, offset: u32) {
        if offset == 0 { return; }
        let off = offset as i32;
        if off <= 127 {
            self.code.extend_from_slice(&[0x48, 0x83, 0xC0, off as u8]);
        } else {
            self.code.extend_from_slice(&[0x48, 0x05]);
            self.code.extend_from_slice(&(off as u32).to_le_bytes());
        }
    }

    /// rax = base_ptr + index * sizeof(elem), donde `base` es una EXPRESION
    /// que produce un puntero (p->arr, a+1...). Deja la direccion en rax.
    pub(super) fn emit_index_ptr_addr(&mut self, base: &Expr, index: &Expr, elem: &TypeSpec) {
        let size = self.type_stack_size(elem).max(1) as u32;
        self.emit_expr(base);          // rax = puntero base
        self.code.push(0x50);          // push base
        self.emit_expr(index);         // rax = indice
        self.emit_scale_index(size);   // rax = indice * size
        self.code.push(0x5A);          // pop rdx = base
        self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
    }

    /// Carga [rax] -> rax con el tamano y signo EXACTOS del elemento.
    /// Antes siempre era `mov rax,[rax]` (8 bytes): leer int[i] traia basura vecina.
    pub(super) fn emit_load_elem(&mut self, elem: &TypeSpec) {
        match elem {
            // agregados: la direccion ES el valor (a.b.c anidado, arrays en structs)
            TypeSpec::Array(_, _) | TypeSpec::StructRef(_) | TypeSpec::UnionRef(_) => {}
            TypeSpec::Char => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x00]), // movsx rax, byte
            TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x00]), // movzx
            TypeSpec::Short => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x00]),
            TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x00]),
            TypeSpec::Int => self.code.extend_from_slice(&[0x48, 0x63, 0x00]), // movsxd rax, dword
            TypeSpec::UnsignedInt | TypeSpec::Float => self.code.extend_from_slice(&[0x8B, 0x00]), // mov eax, dword
            _ => self.code.extend_from_slice(&[0x48, 0x8B, 0x00]), // mov rax, qword
        }
    }

    /// Guarda rdx -> [rax] con el tamano EXACTO del elemento.
    /// Antes un store de 8 bytes a int[i] pisaba el elemento siguiente.
    pub(super) fn emit_store_elem(&mut self, elem: &TypeSpec) {
        match self.type_stack_size(elem) {
            1 => self.code.extend_from_slice(&[0x88, 0x10]),        // mov [rax], dl
            2 => self.code.extend_from_slice(&[0x66, 0x89, 0x10]),  // mov [rax], dx
            4 => self.code.extend_from_slice(&[0x89, 0x10]),        // mov [rax], edx
            _ => self.code.extend_from_slice(&[0x48, 0x89, 0x10]),  // mov [rax], rdx
        }
    }


    /// **`E1 op= E2` con la DIRECCION de `E1` calculada una sola vez.**
    ///
    /// === La secuencia, y por que ese orden ===
    ///
    /// ```text
    ///   direccion de E1  -> rax     UNA vez. Aqui corren los efectos de E1.
    ///   push rax                    la direccion se guarda; nada la vuelve a calcular
    ///   load [rax]       -> rax     el valor viejo, con el tamano exacto del elemento
    ///   push rax
    ///   E2               -> rax     el operando derecho
    ///   pop rdx                     rdx = viejo, rax = derecho
    ///   <op>                        la MISMA secuencia que usa el operador binario
    ///   mov rdx, rax                el resultado, donde el store lo espera
    ///   pop rax                     la direccion guardada
    ///   store rdx -> [rax]
    ///   mov rax, rdx                el valor del assign ES el valor guardado
    /// ```
    ///
    /// ** Los dos `push` no son pereza: son lo que hace correcta la operacion.
    /// Recalcular la direccion para el store es exactamente el bug -- volveria a
    /// ejecutar el `i++` del indice.
    ///
    /// [!] Y `<op>` se pide a la misma funcion que sirve al operador binario, no
    /// a una copia: si manana `>>=` tiene que distinguir el signo, lo hereda. Una
    /// segunda tabla de operaciones seria una segunda tabla donde equivocarse.
    pub(super) fn emit_assign_op(&mut self, lvalue: &Expr, kind: AssignOpKind, rhs: &Expr) {
        // El tipo del elemento decide el ancho del load y del store. Sacarlo del
        // lvalue y no suponer 8 bytes es lo que evita pisar el campo de al lado.
        let elem = self.tipo_del_lvalue(lvalue);

        // 1. La direccion, UNA vez. Los efectos secundarios del lvalue --el
        //    `i++` de `a[i++]`-- ocurren aqui y solo aqui.
        self.emit_lvalue_addr(lvalue);
        self.code.push(0x50); // push direccion

        // 2. El valor viejo.
        self.emit_load_elem(&elem);
        self.code.push(0x50); // push viejo

        // 3. El operando derecho.
        self.emit_expr(rhs);

        // 4. rdx = viejo, rax = derecho -- que es lo que espera `emit_binop`.
        self.code.push(0x5A); // pop rdx
        let unsigned = self.expr_is_unsigned(lvalue) || self.expr_is_unsigned(rhs);
        let op = Self::bytes_de_op(kind, unsigned);
        self.code.extend_from_slice(&op);

        // 5. Guardar en la direccion guardada.
        self.code.extend_from_slice(&[0x48, 0x89, 0xC2]); // mov rdx, rax  (resultado)
        self.code.push(0x58);                             // pop rax       (direccion)
        self.emit_store_elem(&elem);
        self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx  (el valor)
    }

    /// La direccion de un lvalue, sea de la forma que sea. Reusa los mismos
    /// emisores que el resto del fichero: aqui no hay un segundo camino.
    fn emit_lvalue_addr(&mut self, lvalue: &Expr) {
        match lvalue {
            Expr::Subscript(name, index, scale) => {
                self.emit_subscript_addr(name, index, *scale)
            }
            Expr::IndexPtr(base, index, elem) => {
                self.emit_index_ptr_addr(base, index, elem)
            }
            Expr::Field(base, _, off, _) => {
                self.emit_expr_as_ptr(base);
                self.emit_add_offset(*off);
            }
            Expr::Arrow(base, _, off, _) => {
                self.emit_expr(base);
                self.emit_add_offset(*off);
            }
            Expr::Deref(inner) => self.emit_expr(inner),
            // Una variable suelta no llega aqui: no tiene efectos que duplicar,
            // asi que el parser la sigue desazucarando a `v = v op x`.
            otro => self.emit_expr_as_ptr(otro),
        }
    }

    /// El tipo del elemento al que apunta un lvalue.
    fn tipo_del_lvalue(&self, lvalue: &Expr) -> TypeSpec {
        match lvalue {
            Expr::Subscript(name, _, _) => self.elem_type_of(name),
            Expr::IndexPtr(_, _, elem) => elem.clone(),
            Expr::Field(_, _, _, ty) | Expr::Arrow(_, _, _, ty) => ty.clone(),
            Expr::Deref(inner) => self
                .pointee_type(inner)
                .unwrap_or(TypeSpec::Long),
            _ => TypeSpec::Long,
        }
    }

    /// Los bytes de cada operacion, con `rdx` = izquierdo y `rax` = derecho,
    /// resultado en `rax`.
    ///
    /// ** Es la MISMA eleccion que hacen los operadores binarios, y por eso
    /// `/=`, `%=` y `>>=` heredan la correccion de signo de hoy: sin signo va
    /// `xor rdx,rdx` + `div`, con signo `cqo` + `idiv`. Una copia de esta tabla
    /// habria dejado `a[i] /= b` con el bug que `a[i] = a[i] / b` ya no tiene --
    /// que es la definicion de por que una regla vive en un sitio.
    ///
    /// [!] `%` es el unico que necesita cola: `div` deja el cociente en `rax` y
    /// **el resto en `rdx`**, asi que hay que traerlo.
    fn bytes_de_op(kind: AssignOpKind, unsigned: bool) -> Vec<u8> {
        // `mov rcx,rax` + `mov rax,rdx`: los desplazamientos y las divisiones
        // necesitan el derecho en `rcx`/divisor y el izquierdo en `rax`.
        const A_RCX: [u8; 6] = [0x48, 0x89, 0xC1, 0x48, 0x89, 0xD0];
        let mut v = Vec::new();
        match kind {
            AssignOpKind::Add => v.extend_from_slice(&[0x48, 0x01, 0xD0]), // add rax, rdx
            AssignOpKind::Sub => {
                // rax = rdx - rax, y `sub` va al reves: se opera y se trae.
                v.extend_from_slice(&[0x48, 0x29, 0xC2, 0x48, 0x89, 0xD0]);
            }
            AssignOpKind::Mul => v.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]),
            AssignOpKind::BitAnd => v.extend_from_slice(&[0x48, 0x21, 0xD0]),
            AssignOpKind::BitOr => v.extend_from_slice(&[0x48, 0x09, 0xD0]),
            AssignOpKind::BitXor => v.extend_from_slice(&[0x48, 0x31, 0xD0]),
            AssignOpKind::Shl => {
                v.extend_from_slice(&A_RCX);
                v.extend_from_slice(&[0x48, 0xD3, 0xE0]); // shl rax, cl
            }
            AssignOpKind::Shr => {
                v.extend_from_slice(&A_RCX);
                // shr (/5) sin signo, sar (/7) con el.
                v.extend_from_slice(&[0x48, 0xD3, if unsigned { 0xE8 } else { 0xF8 }]);
            }
            AssignOpKind::Div | AssignOpKind::Mod => {
                v.extend_from_slice(&A_RCX);
                if unsigned {
                    v.extend_from_slice(&[0x48, 0x31, 0xD2]); // xor rdx, rdx
                    v.extend_from_slice(&[0x48, 0xF7, 0xF1]); // div rcx
                } else {
                    v.extend_from_slice(&[0x48, 0x99]);       // cqo
                    v.extend_from_slice(&[0x48, 0xF7, 0xF9]); // idiv rcx
                }
                if kind == AssignOpKind::Mod {
                    v.extend_from_slice(&[0x48, 0x89, 0xD0]); // rax = rdx (el resto)
                }
            }
        }
        v
    }

    /// Emit expression as an address (pointer), not as a value
    pub(super) fn emit_expr_as_ptr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(name) => {
                if let Some(&(offset, _)) = self.var_offsets.get(name) {
                    if offset >= -128 && offset <= 127 {
                        self.code.extend_from_slice(&[0x48, 0x8D, 0x45, offset as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                        self.code.extend_from_slice(&(offset as i32).to_le_bytes());
                    }
                } else if self.global_offsets.contains_key(name) {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                    self.global_fixups.push((self.code.len() - 4, name.clone()));
                } else { self.emit_xor_eax(); }
            }
            Expr::Subscript(name, index, scale) => {
                self.emit_subscript_addr(name, index, *scale);
            }
            Expr::IndexPtr(base, index, elem) => {
                self.emit_index_ptr_addr(base, index, elem);
            }
            _ => self.emit_expr(expr),
        }
    }

    /// Tipo al que apunta una expresion de direccion, si se puede deducir.
    ///
    /// Cubre lo que aparece en la practica: una variable puntero o array,
    /// aritmetica de punteros (`p + 1`), y un cast explicito. Cuando no se
    /// puede deducir se devuelve `None` y el `deref` lee 8 bytes, que es el
    /// comportamiento anterior.
    /// A que apunta esta expresion.
    ///
    /// ** DELEGA EN EL JUEZ UNICO (`crate::tipos`). Antes era una segunda
    /// respuesta a la misma pregunta que resolvia `parser/types.rs`, y las dos
    /// sabian cosas distintas: esta conocia la aritmetica de punteros y la
    /// decadencia de arrays, y NO conocia `&x`. La cabecera de `tipos.rs` trae
    /// la tabla entera de lo que sabia cada una.
    ///
    /// [!] Los brazos que vivian aqui --y las lecciones que traian: `*p++` es
    /// `va_arg`, `*tabla[i]` mira dentro, el stride de una fila de matriz-- se
    /// mudaron con su prosa al juez. Ninguno se perdio.
    pub(super) fn pointee_type(&self, expr: &Expr) -> Option<TypeSpec> {
        crate::tipos::apunta_a(self, expr)
    }

    /// Cuantos bytes avanza `+1` sobre esta expresion, si es un puntero.
    /// `None` cuando no lo es o cuando el elemento mide 1 byte (no hace
    /// falta escalar).
    pub(super) fn pointer_scale(&self, expr: &Expr) -> Option<u32> {
        // ** LA MEDIDA LA TIENE QUE DAR EL CODEGEN, NO EL TIPO.
        //
        // Antes: `self.pointee_type(expr)?.stack_size()`. Y
        // `TypeSpec::stack_size` --que es una funcion del AST, sin acceso a
        // ninguna tabla-- contesta **0** para `StructRef` y `UnionRef`, porque
        // desde ahi no hay forma de saber cuanto mide un struct.
        //
        // Con `0`, el `if size > 1` de abajo daba `None` = "esto no es un
        // puntero", y `p + 1` sobre un `struct T *` avanzaba **UN BYTE** en vez
        // de un elemento. No es un caso raro de DOOM: es cualquier recorrido de
        // una tabla de structs con aritmetica en vez de subindice, y el
        // resultado no es un cuelgue -- es leer un registro a caballo entre dos.
        //
        // `type_stack_size` es la misma cuenta CON la tabla `struct_sizes`
        // delante, que es la que ya usan `emit_index_ptr_addr` y
        // `emit_load_elem`. O sea que el subindice acertaba y la suma no,
        // siendo la misma direccion escrita de dos formas.
        let size = self.type_stack_size(&self.pointee_type(expr)?);
        if size > 1 { Some(size) } else { None }
    }
}

/// El codegen contesta la misma pregunta que el parser, con SU tabla.
///
/// * Locales primero y globales despues: es el orden de sombra de C, y es el
/// mismo `var_type_of` que ya usaban `expr_is_float` y `expr_is_unsigned`.
impl crate::tipos::Ambito for Codegen {
    fn tipo_de_variable(&self, nombre: &str) -> Option<TypeSpec> {
        self.var_type_of(nombre)
    }
}
