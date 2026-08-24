//! **VEX: cuatro reales de golpe.** El decodificador de AVX2 del emulador.
//!
//! ## Por que es un fichero y no un trozo de `mod.rs` (L6a, L6b)
//!
//! ** La de L6b: contesta una pregunta distinta. `mod.rs` decodifica la
//! codificacion **clasica** de x86 --prefijos, ModRM, SIB-- y esto decodifica
//! **VEX**, que es otra codificacion entera: dos o tres bytes que llevan
//! dentro lo que antes eran cuatro prefijos sueltos. No es "mas instrucciones":
//! es otra forma de escribirlas.
//!
//! ** Y la de L6a: al entrar AVX2 el 2026-08-23, `emu/mod.rs` paso de 1.689 a
//! 1.856 lineas. Es codigo de HERRAMIENTA --no corre en la maquina-- asi que el
//! trinquete habria admitido un `--motivo`. Se partio igual: una excusa que se
//! puede escribir no es lo mismo que una que hace falta.
//!
//! ## [!] Y `ymm` es un banco APARTE de `xmm`, a proposito
//!
//! En el silicio de verdad los `xmm` son la mitad baja de los `ymm`, y por eso
//! `vzeroupper` existe. Aqui se guardan separados porque el emulador solo tiene
//! que contestar lo que las pruebas preguntan, y mezclarlos costaria mantener
//! una invariante que nadie mira. **El dia que una prueba lea `xmm0` despues de
//! escribir `ymm0`, esa invariante hace falta y este comentario es donde
//! empieza a buscarse.**

use super::*;

impl Machine {
    /// Decodifica un ModRM y devuelve `(reg, destino)`.
    /// **AVX2 sobre `ymm`: cuatro `flotante64` por instruccion.**
    ///
    /// Las formas que emite la tabla de intrinsecos, y solo esas:
    ///
    /// ```text
    ///    C5 FD 10 /r     vmovupd ymm, [mem]      traer cuatro
    ///    C5 FD 11 /r     vmovupd [mem], ymm      dejarlos
    ///    C5 FD 58 /r     vaddpd  ymm, ymm, ymm
    ///    C5 FD 5C /r     vsubpd
    ///    C5 FD 59 /r     vmulpd
    ///    C4 E2 F5 B8 /r  vfmadd231pd ymm, ymm, [mem]
    ///    C5 F8 77        vzeroupper
    /// ```
    ///
    /// ** `vfmadd231pd` lee el destino ANTES de escribirlo: el `231` dice
    /// exactamente eso. Es la operacion de la que esta hecho un producto de
    /// matrices, y la unica de las cuatro que justifica las otras tres.
    /// ** `pub(super)` y no privada: al mudarse de fichero, "privada" dejo de
    /// significar "de esta maquina" y paso a significar "de este modulo". Es la
    /// unica linea que un reparto por L6d no puede dejar igual, y por eso se
    /// dice aqui en vez de cambiarla callando.
    pub(super) fn vex(&mut self, primero: u8) {
        // Con `C5` el segundo byte lleva R, vvvv, L y pp. Con `C4` son dos, y
        // el primero ademas trae el mapa de opcodes.
        let (vvvv, opcode, rex_r) = if primero == 0xC5 {
            let b = self.fetch_u8();
            let vvvv = ((!b >> 3) & 0xF) as usize;
            let r = ((!b >> 7) & 1) as usize;
            (vvvv, self.fetch_u8(), r)
        } else {
            let b1 = self.fetch_u8();
            let b2 = self.fetch_u8();
            let vvvv = ((!b2 >> 3) & 0xF) as usize;
            let r = ((!b1 >> 7) & 1) as usize;
            (vvvv, self.fetch_u8(), r)
        };

        // `vzeroupper` no lleva modrm: pone a cero la mitad alta de todos.
        if opcode == 0x77 {
            for y in self.ymm.iter_mut() {
                y[2] = 0;
                y[3] = 0;
            }
            return;
        }

        let (reg, rm) = self.modrm(rex_r, 0, 0);
        let leer = |m: &Machine, o: &Operand| -> [u64; 4] {
            match o {
                Operand::Reg(i) => m.ymm[*i],
                Operand::Mem(a) => [
                    m.read_u64(*a),
                    m.read_u64(a + 8),
                    m.read_u64(a + 16),
                    m.read_u64(a + 24),
                ],
            }
        };

        match opcode {
            // vmovupd ymm, [mem]  -- traer
            0x10 => self.ymm[reg] = leer(self, &rm),
            // vmovupd [mem], ymm  -- dejar
            0x11 => match rm {
                Operand::Reg(i) => self.ymm[i] = self.ymm[reg],
                Operand::Mem(a) => {
                    let v = self.ymm[reg];
                    for (k, x) in v.iter().enumerate() {
                        self.write_u64(a + (k as u64) * 8, *x);
                    }
                }
            },
            // Las tres aritmeticas: `reg = vvvv <op> rm`, cuatro a la vez.
            0x58 | 0x5C | 0x59 => {
                let a = self.ymm[vvvv];
                let b = leer(self, &rm);
                for k in 0..4 {
                    let x = f64::from_bits(a[k]);
                    let y = f64::from_bits(b[k]);
                    let r = match opcode {
                        0x58 => x + y,
                        0x5C => x - y,
                        _ => x * y,
                    };
                    self.ymm[reg][k] = r.to_bits();
                }
            }
            // vfmadd231pd: `reg = reg + vvvv * rm`. **Lee el destino primero.**
            0xB8 => {
                let acc = self.ymm[reg];
                let a = self.ymm[vvvv];
                let b = leer(self, &rm);
                for k in 0..4 {
                    let r = f64::from_bits(acc[k])
                        + f64::from_bits(a[k]) * f64::from_bits(b[k]);
                    self.ymm[reg][k] = r.to_bits();
                }
            }
            otro => panic!("opcode VEX 0x{:02X} que BMO no emite", otro),
        }
    }
}
