//! **INDICES Y PUNTEROS**: convertir `a[i]`, `*p` y `p + n` en una direccion.
//!
//! === Por que esto es un fichero aparte ===
//!
//! Porque las cinco formas que C ofrece para llegar a un elemento --`a[i]`,
//! `*(a+i)`, `p[i]`, `*(p+i)`, `&a[i]`-- **son la misma cuenta**: una base, un
//! indice y un PASO. Lo unico que cambia es de donde sale cada uno.
//!
//! Repartidas por `emit_expr`, cada una era una rama que calculaba el paso por
//! su cuenta. Juntas, el paso sale de un solo sitio (`pointer_scale`) y la
//! regla se puede leer.
//!
//! === ** Lo que costo tenerlo repartido, y van tres veces ===
//!
//! El PASO es el numero que mas ha fallado de todo el compilador:
//!
//! | | |
//! |---|---|
//! | `p + 1` sobre `struct T *` | avanzaba UN byte |
//! | `p++` sobre cualquier puntero | avanzaba UN byte |
//! | `&c->defaults[i]` | valia CERO |
//!
//! Los tres son la misma cuenta preguntada desde tres sitios distintos, y los
//! tres se arreglaron por separado el mismo dia. Ese es el argumento para que
//! vivan juntos: **la tercera vez que se paga el mismo bug, el arreglo no es el
//! caso que falta, es el reparto**.

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
    pub(super) fn pointee_type(&self, expr: &Expr) -> Option<TypeSpec> {
        match expr {
            Expr::Var(name) => match self.var_type_of(name) {
                Some(TypeSpec::Ptr(inner)) | Some(TypeSpec::Array(inner, _)) => Some(*inner),
                _ => None,
            },
            Expr::Cast(TypeSpec::Ptr(inner), _) => Some((**inner).clone()),
            Expr::Add(a, b) | Expr::Sub(a, b) => {
                self.pointee_type(a).or_else(|| self.pointee_type(b))
            }
            // * `*tabla[i]` -- la tabla es DE PUNTEROS, y hay que mirar dentro.
            //
            // Sin esto el `_ => None` de abajo dejaba caer el deref en el caso
            // por defecto, que lee OCHO bytes. Y ahi no hay error: `*p` sobre
            // un `int*` devolvia el entero pedido **y el de al lado en la mitad
            // alta**, o sea `(20 << 32) | 10` donde tocaba un 10.
            //
            // Se ve en cuanto se ejecuta y no se ve nunca mirando el binario,
            // que es por lo que este banco de pruebas corre los programas.
            Expr::Subscript(name, _, _) => match self.var_type_of(name) {
                Some(TypeSpec::Ptr(elem)) | Some(TypeSpec::Array(elem, _)) => match *elem {
                    TypeSpec::Ptr(dentro) => Some(*dentro),
                    _ => None,
                },
                _ => None,
            },
            Expr::IndexPtr(_, _, elem) => match elem {
                TypeSpec::Ptr(dentro) => Some((**dentro).clone()),
                _ => None,
            },
            // `p->campo` y `p.campo` cuando el campo ES un puntero.
            Expr::Arrow(_, _, _, ft) | Expr::Field(_, _, _, ft) => match ft {
                TypeSpec::Ptr(dentro) => Some((**dentro).clone()),
                _ => None,
            },
            // ** `p++` SIGUE SIENDO UN PUNTERO, Y ESTO FALTABA.
            //
            // Sin este brazo, `*p++` caia en el `_ => None` de abajo: el `Deref`
            // no sabia a que apuntaba y leia **ocho bytes** por defecto. Con un
            // `int *` eso da `(v[1] << 32) | v[0]` -- dos enteros pegados en uno,
            // que es un numero enorme y perfectamente legitimo.
            //
            // [!] Y es EXACTAMENTE la macro `va_arg`. El arreglo de la manana
            // --que `p++` avanzara un elemento-- se probo con
            // `unsigned long long *`, donde el tamano por defecto y el real
            // coinciden en 8: **la prueba pasaba por casualidad**. Lo destapo la
            // sonda del lenguaje al ejercer la misma casilla con `int *`.
            //
            // La leccion, para la proxima: al probar un tamano, **no usar el
            // tipo cuyo tamano es el valor por defecto**.
            Expr::PostInc(n) | Expr::PreInc(n) | Expr::PostDec(n) | Expr::PreDec(n) => {
                match self.var_type_of(n) {
                    Some(TypeSpec::Ptr(inner)) | Some(TypeSpec::Array(inner, _)) => Some(*inner),
                    _ => None,
                }
            }
            _ => None,
        }
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
