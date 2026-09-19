//! **EL VALOR** -- aritmetica, signo, ancho y comparaciones.
//!
//! [fase]     EMISION
//!
//! [aparece]  DENTRO -- si esto emite un byte de mas, el programa **calcula
//!            otra cosa** y sigue corriendo. Los CINCO fallos de codegen del 01
//!            al 04-09 salieron de aqui, y los cinco aparecieron dentro de DOOM
//!
//! [carril]   ROJO -- y NO se elige: sale de su `[aparece]` (DENTRO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//!
//! [cuesta]   DATO -- `9215` en vez de `9216` no rompe nada: rompe la partida
//!
//! [riesgo]   SILENCIO
//!            SILENCIO -- `div` donde iba `idiv`, `shr` donde iba `sar`, un
//!                     recorte de 32 que no se aplica. Ninguno peta. Todos
//!                     mienten
//!
//! # Por que estos brazos y no otros
//!
//! Porque son los que producen un VALOR a partir de otros valores. No tocan
//! una direccion ni deciden a donde saltar: entran numeros y sale un numero.
//! Eso es exactamente lo que los hace silenciosos -- **no hay nada que se pueda
//! romper de forma ruidosa cuando lo unico que haces es contestar**.

use crate::ast::*;

use super::super::Codegen;

impl Codegen {
    /// Los brazos de este carril. El despacho vive en `emitir/mod.rs`
    /// y es EXHAUSTIVO: si manana nace una forma nueva de expresion,
    /// el compilador para alli y no aqui.
    pub(super) fn emitir_valor(&mut self, expr: &Expr) {
        match expr {
            // ** SIETE BYTES EN VEZ DE DIEZ cuando el numero cabe en 32 bits.
            // `48 C7 C0` extiende el signo, que es justo lo que un `i64` que
            // cabe en `i32` necesita. Ver `emit_mov_rax_imm`.
            Expr::Int(n) => self.emit_mov_rax_imm(*n),
            // El guard SSE de arriba ya captura los floats; este brazo solo
            // existe por exhaustividad (defensivo: trunca a entero).
            Expr::FloatLit(_) => {
                self.emit_fexpr(expr);
                self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
            }
            Expr::CharLit(c) => {
                self.emit_mov_rax_imm(*c as i64);
            }
            Expr::Neg(a) => self.emit_unario(a, 0xD8, expr),
            // ** `!x` -- Y AQUI FALTABA EL `movzx`, QUE NO ES DECORATIVO.
            //
            // Era `test eax,eax` + `sete al`, y nada mas. `setcc` **solo escribe
            // `al`**: los 56 bits altos de `rax` se quedan como estaban. Con un
            // operando negativo --`rax = 0xFFFF_FFFF_FFFF_FFFA` para un -6-- el
            // `sete` pone `al = 0` y deja `0xFFFF_FFFF_FFFF_FF00`, o sea
            // **`!(-6)` valia -256**. Que en un `if` es VERDADERO.
            //
            // Lo que eso significa en C de verdad: `if (!strcmp(a, b))` --el
            // idioma mas comun del lenguaje para comparar cadenas-- **acertaba
            // cuando `a` era MENOR que `b`**, porque `strcmp` contesta negativo.
            // En DOOM, `M_CheckParmWithArgs("-config", ...)` casaba con
            // `-iwad` ('c' < 'i'), y por eso el juego anunciaba
            // `saving config in apps/doom1.wad`: iba a escribir su
            // configuracion ENCIMA DEL WAD.
            //
            // [!] La leccion ya estaba aprendida **en este mismo fichero**:
            // `emit_cmp` lleva su `movzx` con un comentario que dice justo esto,
            // *"el movzx del final NO es decorativo"*. Se aprendio en un sitio y
            // no se aplico en el de al lado.
            //
            // Y el `test` pasa a 64 bits (`48 85 C0`) a proposito: `!p` sobre un
            // puntero tiene que mirar el puntero ENTERO. Con `test eax,eax`, una
            // direccion cuyos 32 bits bajos fueran cero se declaraba nula.
            Expr::Not(a) => {
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
                self.code.extend_from_slice(&[0x0F, 0x94, 0xC0]); // sete al
                self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
            }
            Expr::BitNot(a) => self.emit_unario(a, 0xD0, expr),
            // Tras `emit_binop`: rdx = operando IZQUIERDO, rax = DERECHO.
            // Los operadores conmutativos daban igual; los que no lo son
            // estaban invertidos y nadie lo vio hasta ejecutarlos.
            // `p + n` con `p` puntero avanza n ELEMENTOS, no n bytes. Antes
            // sumaba bytes: con `int *p`, `*(p+1)` leia desde el byte 1 en
            // vez del 4, o sea a caballo entre dos enteros.
            Expr::Add(a, b) => {
                if let Some(scale) = self.pointer_scale(a) {
                    let scaled = Expr::Mul(b.clone(), Box::new(Expr::Int(scale as i64)));
                    self.emit_binop(a, &scaled, &[0x48, 0x01, 0xD0]);
                } else if let Some(scale) = self.pointer_scale(b) {
                    let scaled = Expr::Mul(a.clone(), Box::new(Expr::Int(scale as i64)));
                    self.emit_binop(&scaled, b, &[0x48, 0x01, 0xD0]);
                } else {
                    self.emit_binop(a, b, &[0x48, 0x01, 0xD0]);
                }
                self.recortar_a_32(expr);
            }
            // `a - b`. Antes: `sub rax, rdx` = b - a, o sea al reves.
            // `10 - 3` daba -7.
            Expr::Sub(a, b) => {
                const SUB: &[u8] = &[
                    0x48, 0x29, 0xC2, // sub rdx, rax   -> rdx = a - b
                    0x48, 0x89, 0xD0, // mov rax, rdx
                ];
                // `p - n` retrocede n ELEMENTOS.
                match (self.pointer_scale(a), self.pointer_scale(b)) {
                    (Some(scale), None) => {
                        let scaled = Expr::Mul(b.clone(), Box::new(Expr::Int(scale as i64)));
                        self.emit_binop(a, &scaled, SUB);
                    }
                    // ** PUNTERO MENOS PUNTERO DA UN INDICE, NO UNOS BYTES.
                    //
                    // El comentario que habia aqui decia que este caso *"no se
                    // deduce aqui"* -- o sea, hueco reconocido y sin cerrar. Y
                    // el resultado era plausible y equivocado: con `int *`,
                    // `b - a` sobre cinco elementos contestaba **20**.
                    //
                    // Es la cuenta inversa de `p + n`: aquella multiplica por el
                    // tamano del elemento, esta divide. Y la division va CON
                    // SIGNO, porque `a - b` con `a` antes que `b` es negativo y
                    // eso es legal en C -- una division sin signo lo convertiria
                    // en un numero gigante.
                    //
                    // Lo destapo la sonda del lenguaje, no un arranque.
                    (Some(scale), Some(_)) if scale > 1 => {
                        self.emit_binop(a, b, SUB);
                        // mov rcx, scale ; cqo ; idiv rcx
                        self.code.extend_from_slice(&[0x48, 0xC7, 0xC1]);
                        self.code.extend_from_slice(&scale.to_le_bytes());
                        self.code.extend_from_slice(&[0x48, 0x99]);
                        self.code.extend_from_slice(&[0x48, 0xF7, 0xF9]);
                    }
                    _ => self.emit_binop(a, b, SUB),
                }
                self.recortar_a_32(expr);
            }
            Expr::Mul(a, b) => {
                self.emit_binop(a, b, &[0x48, 0x0F, 0xAF, 0xC2]);
                self.recortar_a_32(expr);
            }
            // `a / b` CON SIGNO. Antes hacia dos `pop` habiendo empujado una
            // sola vez --se llevaba un valor de la pila que no era suyo-- y
            // ademas dividia sin signo. `10 / 3` daba 0.
            //
            // ** Y SIN SIGNO es `div` con `rdx` a CERO, no `cqo`+`idiv`.
            // `cqo` extiende el signo de `rax` a `rdx`, o sea que con el bit 63
            // puesto deja `rdx = -1` y la division de 128 bits se hace sobre un
            // dividendo negativo. Ver `expr_is_unsigned`.
            Expr::Div(a, b) => {
                if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) {
                    self.emit_binop(a, b, &[
                        0x48, 0x89, 0xC1, // mov rcx, rax   -> divisor = b
                        0x48, 0x89, 0xD0, // mov rax, rdx   -> dividendo = a
                        0x48, 0x31, 0xD2, // xor rdx, rdx   -> la mitad alta, a cero
                        0x48, 0xF7, 0xF1, // div rcx
                    ])
                } else {
                    self.emit_binop(a, b, &[
                        0x48, 0x89, 0xC1, // mov rcx, rax
                        0x48, 0x89, 0xD0, // mov rax, rdx
                        0x48, 0x99,       // cqo            -> extiende el signo
                        0x48, 0xF7, 0xF9, // idiv rcx
                    ])
                }
            }
            // `a % b`: el resto queda en rdx.
            Expr::Mod(a, b) => {
                if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) {
                    self.emit_binop(a, b, &[
                        0x48, 0x89, 0xC1, // mov rcx, rax
                        0x48, 0x89, 0xD0, // mov rax, rdx
                        0x48, 0x31, 0xD2, // xor rdx, rdx
                        0x48, 0xF7, 0xF1, // div rcx
                        0x48, 0x89, 0xD0, // mov rax, rdx  -> el resto
                    ])
                } else {
                    self.emit_binop(a, b, &[
                        0x48, 0x89, 0xC1, // mov rcx, rax
                        0x48, 0x89, 0xD0, // mov rax, rdx
                        0x48, 0x99,       // cqo
                        0x48, 0xF7, 0xF9, // idiv rcx
                        0x48, 0x89, 0xD0, // mov rax, rdx  -> el resto
                    ])
                }
            }
            // Comparaciones: si algun operando es float -> comisd (setcc unsigned);
            // si no, la comparacion entera de siempre.
            // Comparaciones enteras: todas comparan `a` contra `b` en ese
            // orden y usan el setcc que les toca. Antes `<`, `>` y `>=`
            // comparaban al reves --`1 < 2` daba 0-- porque la comparacion se
            // hacia sobre `b - a` con el setcc de la forma directa.
            Expr::Eq(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x94) } else { self.emit_cmp(a, b, 0x94) },
            Expr::Neq(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x95) } else { self.emit_cmp(a, b, 0x95) },
            // ** Las cuatro de ORDEN llevan DOS `setcc`: con signo y sin el.
            //
            // `setl`/`setb` no son la misma instruccion porque `<` no es la
            // misma pregunta: `0x8000000000000000 > 1` es cierto para un
            // `unsigned long` y falso para un `long`. Las de igualdad (`==`,
            // `!=`) no cambian -- dos patrones de bits son iguales o no lo son,
            // y eso no depende de como se lean.
            //
            // El `setcc` sin signo es el de flotante: `comisd` deja las
            // banderas en la forma no ordenada, y esos son justo los codigos
            // `setb`/`seta`/`setbe`/`setae`. Por eso el brazo de float ya los
            // usaba y el entero no.
            Expr::Lt(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x92) }
                else if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) { self.emit_cmp(a, b, 0x92) }
                else { self.emit_cmp(a, b, 0x9C) },
            Expr::Gt(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x97) }
                else if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) { self.emit_cmp(a, b, 0x97) }
                else { self.emit_cmp(a, b, 0x9F) },
            Expr::Le(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x96) }
                else if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) { self.emit_cmp(a, b, 0x96) }
                else { self.emit_cmp(a, b, 0x9E) },
            Expr::Ge(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x93) }
                else if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) { self.emit_cmp(a, b, 0x93) }
                else { self.emit_cmp(a, b, 0x9D) },
            Expr::BitAnd(a, b) => self.emit_binop(a, b, &[0x48, 0x21, 0xD0]),
            Expr::BitXor(a, b) => self.emit_binop(a, b, &[0x48, 0x31, 0xD0]),
            Expr::BitOr(a, b) => self.emit_binop(a, b, &[0x48, 0x09, 0xD0]),
            // `a << b` / `a >> b`. Antes desplazaban el operando DERECHO por
            // el izquierdo: `1 << 3` intentaba `3 << 1`.
            //
            // A la izquierda no hay dos versiones: `shl` y `sal` son la misma
            // instruccion. A la derecha si -- `sar` copia el bit de signo y
            // `shr` mete ceros -- y **manda el operando IZQUIERDO**, no la
            // conversion usual: `1u >> x` es sin signo aunque `x` sea `int`.
            Expr::Shl(a, b) => self.emit_desplazamiento(a, b, true, expr),
            Expr::Shr(a, b) => self.emit_desplazamiento(a, b, false, expr),
            Expr::Cast(t, inner) => {
                // cast REAL: trunca/extiende rax al tamano del tipo destino.
                // Antes era no-op: (char)300 quedaba como 300.
                self.emit_expr(inner);
                match t {
                    TypeSpec::Char => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0xC0]), // movsx rax, al
                    TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]), // movzx
                    TypeSpec::Short => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0xC0]),
                    TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0xC0]),
                    TypeSpec::Int => self.code.extend_from_slice(&[0x48, 0x63, 0xC0]), // movsxd rax, eax
                    TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x89, 0xC0]), // mov eax, eax (zero-ext)
                    _ => {} // 64-bit y punteros: sin cambio de representacion
                }
            }
            _ => unreachable!("el despacho de emit_expr y este carril no dicen lo mismo"),
        }
    }
}
