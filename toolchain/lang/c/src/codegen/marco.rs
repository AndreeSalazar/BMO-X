//! **EL MARCO DE PILA**: donde vive cada variable local y como se lee.
//!
//! === Por que esto es un fichero aparte ===
//!
//! BMO C no tiene asignacion de registros: **toda variable vive en la pila**,
//! en un hueco fijo de `[rbp+disp]` que se calcula ANTES de emitir una sola
//! instruccion del cuerpo. Eso convierte "las variables" en un subsistema con
//! dos mitades bien separadas --recorrer las declaraciones para repartir el
//! sitio (`build_var_map`), y luego leer y escribir esos sitios-- que no se
//! parecen a nada mas del emisor.
//!
//! === Lo que hay que conservar ===
//!
//! `emit_load_var` es el metodo mas largo del subsistema y no por capricho: es
//! el unico sitio donde el ANCHO del tipo decide la instruccion. Un `int` se
//! carga con `movsxd` y un `unsigned int` con `mov eax`; confundirlos no da un
//! error, da un numero. Ver `sonda_de_anchos` en el banco de pruebas.

use super::*;

impl Codegen {
    /// Recolecta TODAS las DeclAssign del cuerpo, a cualquier profundidad.
    /// Antes solo se miraba el nivel superior: una `int i` dentro de un
    /// for/if/bloque NO recibia slot -- stores descartados, loads = 0.
    pub(super) fn collect_decls_stmt<'a>(s: &'a Stmt, out: &mut Vec<(&'a String, &'a TypeSpec)>) {
        match s {
            Stmt::DeclAssign(t, n, _) => out.push((n, t)),
            // El hueco en la pila. Sin esta linea la variable caia al
            // reparto de legado (8 bytes, tipo Long) y un struct de 16
            // habria escrito sobre la de al lado.
            Stmt::DeclInit(t, n, _) => out.push((n, t)),
            Stmt::Block(v) => for x in v { Self::collect_decls_stmt(x, out); },
            Stmt::If(_, a, b) => {
                Self::collect_decls_stmt(a, out);
                if let Some(b) = b { Self::collect_decls_stmt(b, out); }
            }
            Stmt::While(_, b) | Stmt::DoWhile(b, _) | Stmt::For(_, _, _, b) => Self::collect_decls_stmt(b, out),
            Stmt::Switch(_, cases) => for c in cases { for st in &c.stmts { Self::collect_decls_stmt(st, out); } },
            _ => {}
        }
    }

    pub(super) fn build_var_map(&mut self, params: &[Param], var_names: &[String], func: &Function) {
        self.var_offsets.clear();
        // -- Los parametros, en la pila del llamante --
        //
        // Empiezan en `[rbp+16]` (detras de la direccion de retorno y del `rbp`
        // guardado) y avanzan por RANURAS, no de ocho en ocho: un agregado de
        // 12 bytes ocupa dos y corre el que viene detras.
        //
        // Era `16 + i*8` fijo. Mientras todo cupo en un registro daba lo mismo;
        // el dia que entro un struct por valor, el segundo parametro empezaba a
        // leerse desde la mitad del primero.
        let mut off = 16i32;
        for p in params.iter() {
            self.var_offsets.insert(p.name.clone(), (off, p.typ.clone()));
            let bytes = self.type_stack_size(&p.typ);
            off += agregados::ranuras(bytes) as i32 * 8;
        }
        // locales: tamano REAL del tipo (arrays y structs incluidos), alineado a 8
        let mut decls = Vec::new();
        for stmt in &func.body { Self::collect_decls_stmt(stmt, &mut decls); }
        let mut cur: i32 = 0;
        for (name, typ) in &decls {
            if self.var_offsets.contains_key(*name) { continue; } // sombra: un solo slot
            let sz = self.type_stack_size(typ).max(8);
            let sz = ((sz + 7) / 8 * 8) as i32;
            cur -= sz;
            self.var_offsets.insert((*name).clone(), (cur, (*typ).clone()));
        }
        // legado: nombres registrados por el parser sin DeclAssign visible
        for name in var_names.iter().skip(params.len()) {
            if !self.var_offsets.contains_key(name) {
                cur -= 8;
                self.var_offsets.insert(name.clone(), (cur, TypeSpec::Long));
            }
        }
        self.frame_size = -cur;
    }

    /// Guarda `rax` en `[rbp+disp]` con el tamano EXACTO de `tipo`.
    ///
    /// La pareja de `emit_store_var`, pero por offset en vez de por nombre: una
    /// lista de inicializacion escribe **dentro** de una variable, no sobre
    /// ella. Escribir siempre 8 bytes pisaria el campo siguiente -- es el mismo
    /// bug que ya se pago con `pt.x = 10` cuando `x` era `int`.
    pub(super) fn emit_store_rbp(&mut self, disp: i32, tipo: &TypeSpec) {
        let corto = (-128..=127).contains(&disp);
        let modrm = if corto { 0x45 } else { 0x85 };
        let opcode: &[u8] = match tipo {
            TypeSpec::Char | TypeSpec::UnsignedChar => &[0x88],
            TypeSpec::Short | TypeSpec::UnsignedShort => &[0x66, 0x89],
            TypeSpec::Int | TypeSpec::UnsignedInt | TypeSpec::Float => &[0x89],
            _ => &[0x48, 0x89],
        };
        self.code.extend_from_slice(opcode);
        self.code.push(modrm);
        if corto {
            self.code.push(disp as u8);
        } else {
            self.code.extend_from_slice(&disp.to_le_bytes());
        }
    }

    /// Pone a cero `bytes` bytes a partir de `[rbp+base]`.
    ///
    /// De ocho en ocho mientras quepa, y el resto byte a byte. Sin memset:
    /// aqui no hay libc, y para los tamanos de un struct local un bucle
    /// desenrollado es mas corto que la llamada que no existe.
    pub(super) fn emit_cero_local(&mut self, base: i32, bytes: u32) {
        if bytes == 0 {
            return;
        }
        self.emit_xor_eax();
        let mut hecho = 0u32;
        while bytes - hecho >= 8 {
            self.emit_store_rbp(base + hecho as i32, &TypeSpec::Long);
            hecho += 8;
        }
        while hecho < bytes {
            self.emit_store_rbp(base + hecho as i32, &TypeSpec::Char);
            hecho += 1;
        }
    }

    pub(super) fn emit_store_var(&mut self, name: &str) {
        if let Some(&(offset, ref typ)) = self.var_offsets.get(name) {
            let disp = offset;
            let rex8 = if disp >= -128 && disp <= 127 { 0x45 } else { 0x85 };
            match typ {
                TypeSpec::Char | TypeSpec::UnsignedChar => {
                    self.code.extend_from_slice(&[0x88, rex8]);
                    if disp >= -128 && disp <= 127 { self.code.push(disp as u8); }
                    else { self.code.extend_from_slice(&(disp as i32).to_le_bytes()); }
                }
                TypeSpec::Short | TypeSpec::UnsignedShort => {
                    self.code.extend_from_slice(&[0x66, 0x89, rex8]);
                    if disp >= -128 && disp <= 127 { self.code.push(disp as u8); }
                    else { self.code.extend_from_slice(&(disp as i32).to_le_bytes()); }
                }
                TypeSpec::Int | TypeSpec::UnsignedInt => {
                    if disp >= -128 && disp <= 127 {
                        self.code.extend_from_slice(&[0x89, 0x45, disp as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x89, 0x85]);
                        self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                    }
                }
                _ => {
                    if disp >= -128 && disp <= 127 {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x45, disp as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
                        self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                    }
                }
            }
        } else if let Some(&(_, ref typ)) = self.global_offsets.get(name) {
            // rax already has value; lea rdi, [rip+0]; mov [rdi], reg
            self.code.extend_from_slice(&[0x48, 0x8D, 0x3D, 0, 0, 0, 0]);
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            match typ {
                TypeSpec::Char | TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x88, 0x07]),
                TypeSpec::Short | TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x66, 0x89, 0x07]),
                TypeSpec::Int | TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x89, 0x07]),
                _ => self.code.extend_from_slice(&[0x48, 0x89, 0x07]),
            }
        }
    }

    pub(super) fn emit_load_var(&mut self, name: &str) {
        // Enum constants: emit integer literal directly
        if let Some(&val) = self.enum_values.get(name) {
            self.code.extend_from_slice(&[0xB8]); // mov eax, imm32
            self.code.extend_from_slice(&(val as i32).to_le_bytes());
            return;
        }
        // Funcion usada como VALOR (fp = myfunc): decae a su direccion.
        if self.known_functions.contains(name)
            && !self.var_offsets.contains_key(name)
            && !self.global_offsets.contains_key(name)
        {
            self.emit_func_addr(name);
            return;
        }
        // Arrays: decaen a puntero -- "cargar" arr es su DIRECCION, no su contenido
        if self.var_is_array(name) {
            if let Some(&(off, _)) = self.var_offsets.get(name) {
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x45, off as u8]); // lea rax,[rbp+off]
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                    self.code.extend_from_slice(&off.to_le_bytes());
                }
            } else {
                self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                self.global_fixups.push((self.code.len() - 4, name.to_string()));
            }
            return;
        }
        if let Some(&(offset, ref typ)) = self.var_offsets.get(name) {
            let disp = offset;
            match typ {
            TypeSpec::Char => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::UnsignedChar => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::Short => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::UnsignedShort => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            // Un `int` con signo debe EXTENDER EL SIGNO al leerse: el resto
            // del codegen trabaja en 64 bits. Antes usaba `mov eax, [..]`,
            // que rellena de ceros, asi que un `int y = -7;` se releia como
            // 4294967289. Los tipos mas chicos ya lo hacian bien (movsx);
            // solo `int` se habia quedado sin su version con signo.
            TypeSpec::Int => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x63, 0x45, disp as u8]); // movsxd
                } else {
                    self.code.extend_from_slice(&[0x48, 0x63, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::UnsignedInt => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x8B, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x8B, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            _ => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8B, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
        }
        } else if let Some(&(_, ref typ)) = self.global_offsets.get(name) {
            // lea rax, [rip+0]; then mov with size to load value
            self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            match typ {
                TypeSpec::Char => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x00]),
                TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x00]),
                TypeSpec::Short => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x00]),
                TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x00]),
                // * `int` con SIGNO se extiende con signo, y compartia arm con
                // `unsigned int`.
                //
                // Era `mov eax,[rax]` para los dos, que rellena de CEROS los 32
                // bits altos. `char` y `short` si usaban `movsx` --asi que la
                // intencion estaba clara y el `int` se quedo fuera--, y no se
                // notaba porque **ningun global podia valer negativo**: el
                // inicializador solo entendia `Expr::Int` positivo y todo lo
                // demas se rellenaba de ceros en silencio. Al arreglar aquello,
                // `int frio = -40;` empezo a imprimir **4294967256**.
                //
                // `movsxd rax, dword [rax]` = `48 63 00`.
                TypeSpec::Int => self.code.extend_from_slice(&[0x48, 0x63, 0x00]),
                TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x8B, 0x00]),
                _ => self.code.extend_from_slice(&[0x48, 0x8B, 0x00]),
            }
        } else {
            // * Un nombre que no es variable, ni global, ni constante de enum,
            // ni funcion, NO VALE CERO: no existe.
            //
            // Esto era un `xor eax,eax` mudo, y es lo que escondio que
            // `#include` tiraba los `#define` de la cabecera: `BMO_TECLA_REPAG`
            // y `BMO_TECLA_AVPAG` llegaban sin expandir, el codegen los ponia a
            // cero **a los dos**, y `if (t == REPAG)` era cierto para AvPag.
            // Comparaba cero contra cero y el programa parecia correcto.
            //
            // Un cero inventado es la peor respuesta posible a "no se que es
            // esto": es un valor legitimo en cualquier expresion, asi que el
            // error viaja hasta donde ya no se puede rastrear.
            self.errors.push(format!(
                "'{name}' no esta declarado (ni variable, ni global, ni constante de enum, \
                 ni funcion). Si venia de un #define, la cabecera no llego a expandirse."
            ));
            self.emit_xor_eax();
        }
    }

    /// Deja el handle (`rdx` de la primera llamada, hoy perdido) y la base
    /// (`rax`) en las globales que `<bmo/archivo.h>` declara.
    ///
    /// Se llama con **`rax` = base** y con el handle todavia recuperable: no lo
    /// esta, asi que hay que haberlo guardado antes. Ver el uso en `malloc`.
    ///
    /// Si el programa no declara esas globales, esto no emite **nada**: un
    /// programa que no lee ficheros no debe pagar por la maquinaria de los que
    /// si. Por eso se pregunta por el nombre en vez de reservarlas siempre.
    /// *** SE PUBLICA EL **PRIMER** BLOQUE, NO EL ULTIMO.
    ///
    /// Esto publicaba el bloque de cada `malloc`, pisando el anterior, y de ahi
    /// salia una mina que solo se ve cuando ya mordio:
    ///
    /// `fread` calcula `desde = dst - base` y el kernel escribe en
    /// `base + desde`. Los bloques se entregan **seguidos y ascendentes** desde
    /// `0xE000_0000`, asi que si `base` es el del ultimo `malloc` y `dst` esta
    /// en uno ANTERIOR, la resta da negativo -- que sin signo es un numero
    /// enorme, y el kernel lo rechaza por rango. **Devuelve cero, no falla**:
    /// un `fread` que no lee y no se queja.
    ///
    /// O sea que funcionaba o no **segun el orden en que se hubieran pedido los
    /// bloques**, y el orden lo decide quien escribe el programa sin saber que
    /// esta decidiendo nada. `leer_C.c` acertaba por casualidad --abre y luego
    /// pide-- y `<bmo/paquete.h>` fallo a la primera por hacerlo al reves.
    ///
    /// Con el PRIMERO, `desde` es positivo para cualquier direccion que haya
    /// dado `malloc`, y la comprobacion del kernel --que mide contra lo
    /// entregado al PROCESO entero, no a un bloque-- lo acepta. La regla deja de
    /// depender del orden.
    pub(super) fn publicar_bloque(&mut self) {
        for (name, reg) in [("__bmo_bloque_base", 0u8), ("__bmo_bloque_cap", 1u8)] {
            if !self.global_offsets.contains_key(name) {
                continue;
            }
            // lea rdi, [rip+0]  (el fixup pone la direccion de la global)
            self.code.extend_from_slice(&[0x48, 0x8D, 0x3D, 0, 0, 0, 0]);
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            // Ya hay algo publicado? Entonces no se toca.
            //   cmp qword [rdi], 0 ; jne +3
            self.code.extend_from_slice(&[0x48, 0x83, 0x3F, 0x00]); // cmp [rdi], 0
            self.code.extend_from_slice(&[0x75, 0x03]); // jne  (salta el mov de 3 bytes)
            if reg == 0 {
                self.code.extend_from_slice(&[0x48, 0x89, 0x07]); // mov [rdi], rax
            } else {
                self.code.extend_from_slice(&[0x4C, 0x89, 0x07]); // mov [rdi], r8
            }
        }
    }

    pub(super) fn emit_xor_eax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
    }

    /// **Cuanto avanza un `++` sobre esta variable.**
    ///
    /// Uno para todo lo que no sea un puntero; el tamano del APUNTADO para los
    /// que si lo son. Es la regla de C de siempre --`p + 1` avanza un
    /// ELEMENTO-- y hasta el 2026-08-13 este camino no la cumplia: `emit_inc_var`
    /// hacia `add rax, 1` pasara lo que pasara.
    ///
    /// ## Lo que costo, y es lo mas comun que hay en C
    ///
    /// `<stdarg.h>` define `va_arg` asi:
    ///
    /// ```c
    /// #define va_arg(ap, type)   ((type)(*(ap)++))
    /// ```
    ///
    /// O sea que **`va_arg` ES un `*p++`**. Con el paso de un byte, la primera
    /// lectura acierta --el puntero sigue donde se puso-- y de la segunda en
    /// adelante se lee a caballo entre dos casillas. En DOOM eso fue
    /// `M_StringJoin` recorriendo 19 punteros basura en vez de 3 y haciendoles
    /// `strlen`: `#PF` y tarea eliminada.
    ///
    /// Y fuera de DOOM es peor, porque `while (*p) p++;` es el idioma mas comun
    /// del lenguaje.
    ///
    /// [!] ** ES EL TERCER BRAZO DE LA MISMA CUENTA EN UN DIA.** `Expr::Add` lo
    /// escalaba por `pointer_scale` --que a su vez media con la funcion
    /// equivocada-- y `Expr::PostInc` no lo escalaba en absoluto. Tres sitios
    /// distintos calculando "cuanto avanza un puntero", y solo uno bien. Cuando
    /// esto vuelva a aparecer, el arreglo no es el caso que falta: es juntar la
    /// cuenta en un sitio.
    pub(super) fn paso_de_puntero(&self, name: &str) -> u32 {
        match self.var_type_of(name) {
            // `Array` no entra a proposito: `arr++` no es C valido, y aceptarlo
            // aqui seria inventarse una semantica que el parser no promete.
            Some(TypeSpec::Ptr(inner)) => self.type_stack_size(&inner).max(1),
            _ => 1,
        }
    }

    /// `add rax, paso` con la codificacion corta cuando cabe.
    pub(super) fn emit_suma_paso(&mut self, paso: u32, restar: bool) {
        let op8 = if restar { 0xE8 } else { 0xC0 };
        if paso <= 127 {
            self.code.extend_from_slice(&[0x48, 0x83, op8, paso as u8]);
        } else {
            // REX.W + 05/2D id -- `add/sub rax, imm32`.
            self.code.extend_from_slice(&[0x48, if restar { 0x2D } else { 0x05 }]);
            self.code.extend_from_slice(&paso.to_le_bytes());
        }
    }

    pub(super) fn emit_inc_var(&mut self, name: &str) {
        if !self.var_offsets.contains_key(name) && !self.global_offsets.contains_key(name) { self.emit_xor_eax(); return; }
        let paso = self.paso_de_puntero(name);
        self.emit_load_var(name);
        self.emit_suma_paso(paso, false);
        self.emit_store_var(name);
    }

    pub(super) fn emit_dec_var(&mut self, name: &str) {
        if !self.var_offsets.contains_key(name) && !self.global_offsets.contains_key(name) { self.emit_xor_eax(); return; }
        let paso = self.paso_de_puntero(name);
        self.emit_load_var(name);
        self.emit_suma_paso(paso, true);
        self.emit_store_var(name);
    }
}
