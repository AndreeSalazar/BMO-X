//! **EL ORDEN** -- llamadas, cortocircuitos y secuencia.
//!
//! [fase]     EMISION
//!
//! [aparece]  METAL -- un salto mal puesto se ve al primer intento, casi
//!            siempre con una excepcion y un `rip` que se puede mirar con
//!            `--map`. Es el mas barato de los tres de encontrar
//!
//! [carril]   AMARILLO -- y NO se elige: sale de su `[aparece]` (METAL).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//!
//! [cuesta]   TAREA -- el programa se cae, y se cae donde esta el fallo
//!
//! [riesgo]   AJENO
//!            AJENO -- un `Intrinsic` cruza la puerta: lo que pasa
//!                     al otro lado no lo decide este fichero, y un numero de
//!                     operacion equivocado lo rechaza el KERNEL, no el
//!                     compilador
//!
//! # Por que estos brazos y no otros
//!
//! Porque ninguno calcula un valor ni una direccion: **deciden que se ejecuta y
//! en que orden**. `&&` y `||` con sus cortocircuitos, el ternario, la coma, y
//! las tres formas de llamar. Un error aqui rompe la SECUENCIA, y una secuencia
//! rota se ve.

use crate::ast::*;

use super::super::{CallReloc, Codegen, TargetProfile};

impl Codegen {
    /// Los brazos de este carril. El despacho vive en `emitir/mod.rs`
    /// y es EXHAUSTIVO: si manana nace una forma nueva de expresion,
    /// el compilador para alli y no aqui.
    pub(super) fn emitir_orden(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(name, args) => {
                // * Las funciones de biblioteca que se emiten EN LINEA.
                //
                // No hay libreria que enlazar, y no es una carencia: es el
                // modelo. Un `.bex` es una imagen entera y BEF no resuelve
                // relocaciones contra un `.so`. Emitir el bucle cuesta treinta
                // bytes y ahorra un enlazador, un formato de libreria y un
                // cargador dinamico.
                //
                // Lo que se emite vive en `bmo_lower::memoria` (L1) porque
                // "mueve estos bytes" no tiene semantica de lenguaje: COBOL
                // mueve grupos y Ada asigna arrays con la misma emision. Aqui
                // solo se pone el nombre que usa C.
                if let Some(n) = self.emitir_biblioteca(name, args) {
                    let _ = n;
                    return;
                }
                // Special case: printf -> emit bmo_printf from userland_ring3
                if name == "printf" && !args.is_empty() {
                    self.emit_printf_variadic(args);
                    return;
                }
                // La pareja de `printf`: se emiten EN LINEA por lo mismo -- aqui
                // no hay libc que enlazar ni simbolo que nadie resuelva.
                if name == "getchar" && args.is_empty() {
                    self.emit_getchar();
                    return;
                }
                if name == "scanf" && !args.is_empty() {
                    self.emit_scanf(args);
                    return;
                }
                // Llamada INDIRECTA? El nombre no es una funcion pero SI una
                // variable -> contiene una direccion (puntero a funcion).
                let is_indirect = !self.known_functions.contains(name)
                    && (self.var_offsets.contains_key(name) || self.global_offsets.contains_key(name));

                // -- Los argumentos, de derecha a izquierda --
                //
                // Cuantas ranuras ocupa cada uno lo dice el PARAMETRO, no la
                // expresion: un `struct` de 12 bytes ocupa dos aunque quien lo
                // pase sea una variable. Si no hay firma --llamada indirecta por
                // puntero-- se supone una ranura, que es lo que era antes.
                let tipos_param: Vec<TypeSpec> = self
                    .firmas
                    .get(name)
                    .map(|(p, _)| p.clone())
                    .unwrap_or_default();
                // ** LA CONVENCION HIBRIDA (2026-09-19): escalares en registro,
                // agregados y flotantes por la pila, y TODO por la pila si la
                // funcion es variadica. Ver `decidir/llamada.rs` y `argumentos.rs`.
                let de_registro: Vec<bool> = (0..args.len())
                    .map(|i| match tipos_param.get(i) {
                        Some(t) => !self.es_agregado(t) && !Self::is_float_ty(t),
                        None => true,
                    })
                    .collect();
                let pasos = super::super::decidir::llamada::clasificar(&de_registro, self.variadicas.contains(name));
                let ranuras_total = self.emit_argumentos_pila(args, &tipos_param, &pasos);
                self.emit_argumentos_registro(args, &pasos);
                // Devolver un agregado es un tercer mecanismo (puntero oculto)
                // y todavia no esta. Se dice: devolver ocho bytes de un struct
                // de doce seria la clase de mentira que este compilador no
                // cuenta.
                if let Some((_, ret)) = self.firmas.get(name) {
                    if self.es_agregado(&ret.clone()) {
                        self.errors.push(format!(
                            "'{name}' devuelve un struct por valor, y eso aun no se compila \
                             (pasa un puntero al destino como parametro)"
                        ));
                    }
                }
                if is_indirect {
                    self.emit_load_var(name);                 // rax = direccion
                    self.code.extend_from_slice(&[0xFF, 0xD0]); // call rax
                } else {
                    // call rel32 placeholder (directa)
                    self.code.extend_from_slice(&[0xE8]);
                    self.call_relocs.push(CallReloc { offset: self.code.len(), target: name.clone() });
                    self.code.extend_from_slice(&[0, 0, 0, 0]);
                    // Track stdlib imports for Ring 3 apps
                    if self.target == TargetProfile::Ring3App && !self.function_offsets.contains_key(name) {
                        self.stdlib_imports.insert(name.clone());
                    }
                }
                // Se quita de la pila lo que se PUSO, que ya no es una ranura
                // por argumento.
                let n = ranuras_total * 8;
                if n > 0 {
                    if n <= 127 {
                        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, n as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x81, 0xC4]);
                        self.code.extend_from_slice(&n.to_le_bytes());
                    }
                }
            }
            Expr::CallPtr(callee, args) => {
                // *** `(*f)(x)` ES `f(x)`. Desreferenciar un puntero a funcion
                // es un NO-OP, y aqui se emitia como una carga de memoria.
                //
                // C11 6.5.3.2p4: si el operando de `*` apunta a una funcion, el
                // resultado es un DESIGNADOR DE FUNCION. Y 6.5.2.2p1 exige que
                // lo llamado sea un puntero a funcion, asi que ese designador
                // vuelve a decaer inmediatamente. Las tres formas son la misma:
                //
                //     f(x)      (*f)(x)      (**f)(x)
                //
                // ** Sin pelar el `*`, `emit_expr` cargaba OCHO BYTES DE LA
                // DIRECCION DE LA FUNCION --o sea el principio de su codigo-- y
                // llamaba a eso. El metal lo dijo con todas las letras el
                // 2026-09-03, con DOOM ya jugandose:
                //
                //     #GP  ff d0  -> wipe_ScreenWipe+0xa8
                //     *** PUNTERO NO CANONICO: bits 63:48 no copian el bit 47
                //
                // `f_wipe.c:282` escribe `rc = (*wipes[wipeno*3+1])(w, h, t)`,
                // que es el estilo K&R de toda la vida. La tabla estaba BIEN
                // --sus reubicaciones funcionan, `t[0](10)` acierta-- y lo que
                // fallaba era la estrella.
                //
                // [!] Se pela en bucle: `(**f)(x)` es igual de legal.
                let mut destino: &Expr = callee;
                while let Expr::Deref(dentro) = destino {
                    destino = dentro;
                }
                // A traves de un puntero no hay firma: todos escalares, y NO
                // variadica (tomar la direccion de una variadica es un error
                // de compilacion, ver `decidir/llamada.rs`).
                let pasos = super::super::decidir::llamada::clasificar(&vec![true; args.len()], false);
                let ranuras = self.emit_argumentos_pila(args, &[], &pasos);
                // El destino: una variable se carga DESPUES de los registros
                // (solo toca rax); cualquier otra expresion podria pisarlos y
                // se aparca en la pila.
                let destino_simple = matches!(destino, Expr::Var(_));
                if !destino_simple {
                    self.emit_expr(destino);
                    self.code.push(0x50); // push direccion
                }
                self.emit_argumentos_registro(args, &pasos);
                if destino_simple {
                    self.emit_expr(destino);                // rax = direccion de la funcion
                } else {
                    self.code.push(0x58);                   // pop rax
                }
                self.code.extend_from_slice(&[0xFF, 0xD0]); // call rax
                let n = ranuras * 8;
                if n > 0 {
                    if n <= 127 { self.code.extend_from_slice(&[0x48, 0x83, 0xC4, n as u8]); }
                    else { self.code.extend_from_slice(&[0x48, 0x81, 0xC4]); self.code.extend_from_slice(&n.to_le_bytes()); }
                }
            }
            // `&&` y `||` valen 0 o 1, no "el operando que quedo". Antes
            // `0 || 3` daba 3: cortocircuitaba bien pero devolvia el valor
            // crudo, y el estandar dice que el resultado es `int` 0/1.
            Expr::LAnd(a, b) => {
                let end = self.fresh_label();
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x85, 0xC0]);
                self.emit_jz_reloc(end);
                self.emit_expr(b);
                self.resolve_label(end);
                self.emit_normalize_bool();
            }
            Expr::LOr(a, b) => {
                let end = self.fresh_label();
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x85, 0xC0]);
                self.emit_jnz_reloc(end);
                self.emit_expr(b);
                self.resolve_label(end);
                self.emit_normalize_bool();
            }
            Expr::Conditional(c, t, f) => {
                let else_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.emit_test_cond(c, else_lbl);
                self.emit_expr(t);
                self.emit_jmp_reloc(end_lbl);
                self.resolve_label(else_lbl);
                self.emit_expr(f);
                self.resolve_label(end_lbl);
            }
            Expr::Intrinsic(name, args) => self.emit_intrinsic(name, args),
            Expr::Comma(exprs) => {
                for (i, e) in exprs.iter().enumerate() {
                    self.emit_expr(e);
                    if i < exprs.len() - 1 { self.emit_drop(); }
                }
            }
            _ => unreachable!("el despacho de emit_expr y este carril no dicen lo mismo"),
        }
    }
}
