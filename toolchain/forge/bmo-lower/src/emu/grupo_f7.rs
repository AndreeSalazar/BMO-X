//! **El grupo F7**: `not`, `neg`, `mul`, `div` e `idiv`, los cinco que
//! comparten opcode y se distinguen por `/ext`.
//!
//! ** Salio de `mod.rs` el 2026-09-16 por L6a: el despachador paso de las
//! 1.000 lineas de codigo al entrar `mul` (la quinta familia del fallo del
//! signo de INTI) y el DIRECTOR de mentira. Es un brazo del `match` movido
//! como texto, con las mismas variables: `rex_x`, `rex_b`, `wide` y `ancho`
//! llegan como llegaban.

use super::{Machine, RAX, RDX};

impl Machine {
    pub(super) fn grupo_f7(&mut self, rex_x: usize, rex_b: usize, wide: bool, ancho: usize) {
        let (ext, src) = self.modrm(0, rex_x, rex_b);
        let v = self.load(src, wide);
        match ext & 7 {
            // `~x`. Faltaba, y el hueco era invisible: el codegen de C
            // lo emitia BIEN desde siempre --un `.bef` con `~0` se
            // escribe sin quejarse-- pero **ninguna matriz lo podia
            // ejecutar**, asi que ni C ni COBOL tenian una fila con
            // `~`. Lo destapo C++ al escribir la suya desde cero.
            //
            // * A diferencia de `neg`, `not` **no toca las banderas**
            // en x86-64. Llamar a `flags_logic` aqui seria un no-op
            // silencioso el 99% de las veces y una mentira el 1%: un
            // `~x` seguido de un salto condicional decidiria por el
            // resultado del `not` en vez de por la comparacion de
            // antes, que es lo que el silicio conserva.
            2 => {
                let r = !v;
                self.store(src, r, ancho);
            }
            3 => {
                let r = (self.load(src, wide) as i64).wrapping_neg() as u64;
                self.flags_logic(r);
                self.store(src, r, ancho);
            }
            // mul SIN signo: `rdx:rax = rax * operando`. CF y OF se
            // encienden solo si la mitad alta no es cero -- que es lo
            // que el `jc` de la Regla 1 sin signo lee. Como el silicio,
            // no toca `zf` ni `sf`. (2026-09-16: la quinta familia del
            // fallo del signo; ver `operaciones.rs` de INTI)
            4 => {
                let a = self.regs[RAX];
                let (lo, hi) = if wide {
                    let r = (a as u128) * (v as u128);
                    (r as u64, (r >> 64) as u64)
                } else {
                    let r = (a as u32 as u64) * (v as u32 as u64);
                    (r & 0xFFFF_FFFF, r >> 32)
                };
                self.regs[RAX] = lo;
                self.regs[RDX] = hi;
                self.cf = hi != 0;
                self.of = hi != 0;
            }
            // div SIN signo: rdx:rax entre el operando. El emisor
            // siempre pone rdx=0 antes, asi que basta con rax.
            6 => {
                assert_ne!(v, 0, "division por cero en el codigo emitido");
                assert_eq!(
                    self.regs[RDX], 0,
                    "div de 128 bits: el emisor debe poner rdx=0 antes"
                );
                let dividend = self.regs[RAX];
                self.regs[RAX] = dividend / v;
                self.regs[RDX] = dividend % v;
            }
            7 => {
                // idiv: dividendo en rdx:rax; aqui basta rax con signo
                // extendido por cqo, que es lo unico que emitimos.
                let divisor = v as i64;
                assert_ne!(divisor, 0, "division por cero en el codigo emitido");
                let dividend = self.regs[RAX] as i64;
                self.regs[RAX] = dividend.wrapping_div(divisor) as u64;
                self.regs[RDX] = dividend.wrapping_rem(divisor) as u64;
            }
            other => panic!("grupo F7 /{other} no emitido por BMO"),
        }
    }
}
