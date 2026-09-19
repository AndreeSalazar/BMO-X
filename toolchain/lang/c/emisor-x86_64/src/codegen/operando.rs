//! **EL OPERANDO DERECHO SIN PILA**: `x + y` con `y` una variable es
//! `cargar y en rcx ; add rax, rcx`, no empujar `x`, cargar `y`, sacar `x`.
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- si esto carga `y` con la anchura o el signo
//!            equivocados, el programa suma otro numero y el banco lo ve en la
//!            primera fila que opere con un `char` negativo o un `unsigned`
//!            grande
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//! # Lo que dijo el metro (2026-09-18, por la noche)
//!
//! Con el inmediato, el salto fundido y el troquel, el 22 % de lo que
//! ejecutaba C seguia siendo `push`/`pop`, y `--caliente` dijo de que: de
//! `emit_binop` cuando el derecho es una VARIABLE, y de la direccion de
//! `t[i]`. Los dos empujan lo que ya tienen para calcular lo siguiente, y lo
//! siguiente es cargar una variable -- que no toca `rax`.
//!
//! ```text
//!    antes    emitir x ; push rax ; cargar y en rax ; pop rdx ; add rax, rdx    5
//!    ahora    emitir x ; cargar y en rcx ; add rax, rcx                          3
//!    troquel  emitir x ; add rax, r12                                            2
//! ```
//!
//! # *** LA INVARIANTE: cargar en rcx deja LO MISMO que cargar en rax
//!
//! `frame.rs::emit_load_var` es quien sabe como se lee cada variable: `movsxd`
//! para un `int`, `movzx` para un `unsigned char`, `lea` para un array o una
//! funcion. Esto es la misma tabla con `rcx` (o `rdx`) por destino, y por eso
//! los brazos van en el mismo orden. Si aquel cambia, este cambia.
//!
//! # Quien usa que registro, y por que dos
//!
//! ```text
//!    rcx   el operando derecho de una operacion binaria (aqui)
//!    rdx   la BASE de `t[i]` y la DIRECCION de `t[i] = v` (indexing.rs)
//! ```
//!
//! Son dos porque `t[i] = x + y` calcula la direccion, la aparca en `rdx`, y
//! DESPUES evalua `x + y`, que carga `y` en `rcx`. Con un solo registro el
//! valor pisaria la direccion. `sin_pila` (types.rs) es la lista de lo que
//! solo toca `rax` y `rcx`; lo que no esta en ella sigue por la pila.

use crate::ast::TypeSpec;

use super::Codegen;

/// Los dos registros en los que esto sabe cargar. El numero es el campo
/// `reg` del ModRM (rcx = 1, rdx = 2).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Destino {
    Rcx = 1,
    Rdx = 2,
}

impl Codegen {
    /// Carga la variable `name` en `dst` con la anchura y el signo de su tipo,
    /// SIN tocar `rax`. `false` si no es una variable que esto sepa cargar
    /// (entonces el llamante sigue por el camino de siempre).
    pub(super) fn emit_cargar_en(&mut self, name: &str, dst: Destino) -> bool {
        let d = dst as u8;
        // reg = d, rm = [rbp + disp]: mod 01 (disp8) o 10 (disp32)
        let modrm8 = 0x40 | (d << 3) | 5;
        let modrm32 = 0x80 | (d << 3) | 5;
        let rip = (d << 3) | 5; // mod 00, rm 101 = [rip + disp32]

        // El troquel: `mov rdx, rN` = 49 8B (11 d rN)
        if let Some(&r) = self.var_regs.get(name) {
            self.code.extend_from_slice(&[0x49, 0x8B, 0xC0 | (d << 3) | (r - 8)]);
            return true;
        }
        if let Some(&val) = self.enum_values.get(name) {
            // mov ecx/edx, imm32 = B9/BA id
            self.code.push(0xB8 + d);
            self.code.extend_from_slice(&(val as i32).to_le_bytes());
            return true;
        }
        if (self.known_functions.contains(name) || self.solo_prototipo(name))
            && !self.var_offsets.contains_key(name)
            && !self.global_offsets.contains_key(name)
        {
            self.code.extend_from_slice(&[0x48, 0x8D, rip, 0, 0, 0, 0]); // lea dst, [rip+f]
            self.func_addr_fixups.push((self.code.len() - 4, name.to_string()));
            return true;
        }
        if self.var_is_array(name) {
            if let Some(&(off, _)) = self.var_offsets.get(name) {
                self.code.extend_from_slice(&[0x48, 0x8D]); // lea dst, [rbp+off]
                self.emit_modrm_rbp(modrm8, modrm32, off);
            } else {
                self.code.extend_from_slice(&[0x48, 0x8D, rip, 0, 0, 0, 0]); // lea dst, [rip+g]
                self.global_fixups.push((self.code.len() - 4, name.to_string()));
            }
            return true;
        }
        if let Some((off, typ)) = self.var_offsets.get(name).map(|(o, t)| (*o, t.clone())) {
            let op: &[u8] = match typ {
                TypeSpec::Char => &[0x48, 0x0F, 0xBE],
                TypeSpec::UnsignedChar => &[0x48, 0x0F, 0xB6],
                TypeSpec::Short => &[0x48, 0x0F, 0xBF],
                TypeSpec::UnsignedShort => &[0x48, 0x0F, 0xB7],
                TypeSpec::Int => &[0x48, 0x63],
                TypeSpec::UnsignedInt => &[0x8B],
                _ => &[0x48, 0x8B],
            };
            self.code.extend_from_slice(op);
            self.emit_modrm_rbp(modrm8, modrm32, off);
            return true;
        }
        if let Some(typ) = self.global_offsets.get(name).map(|(_, t)| t.clone()) {
            // lea dst, [rip+g] ; luego la carga desde [dst]
            self.code.extend_from_slice(&[0x48, 0x8D, rip, 0, 0, 0, 0]);
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            let desde = (d << 3) | d; // mod 00, reg dst, rm dst = [dst]
            let op: &[u8] = match typ {
                TypeSpec::Char => &[0x48, 0x0F, 0xBE],
                TypeSpec::UnsignedChar => &[0x48, 0x0F, 0xB6],
                TypeSpec::Short => &[0x48, 0x0F, 0xBF],
                TypeSpec::UnsignedShort => &[0x48, 0x0F, 0xB7],
                TypeSpec::Int => &[0x48, 0x63],
                TypeSpec::UnsignedInt => &[0x8B],
                _ => &[0x48, 0x8B],
            };
            self.code.extend_from_slice(op);
            self.code.push(desde);
            return true;
        }
        false
    }

    /// Sabe `emit_cargar_en` cargar este nombre? Se pregunta ANTES de emitir
    /// el operando izquierdo, para no emitirlo dos veces si la respuesta es no.
    pub(super) fn sabe_cargar(&self, name: &str) -> bool {
        self.var_regs.contains_key(name)
            || self.enum_values.contains_key(name)
            || self.known_functions.contains(name)
            || self.solo_prototipo(name)
            || self.var_offsets.contains_key(name)
            || self.global_offsets.contains_key(name)
    }

    /// El ModRM + desplazamiento de `[rbp + off]`, corto si cabe.
    fn emit_modrm_rbp(&mut self, modrm8: u8, modrm32: u8, off: i32) {
        if (-128..=127).contains(&off) {
            self.code.extend_from_slice(&[modrm8, off as u8]);
        } else {
            self.code.push(modrm32);
            self.code.extend_from_slice(&off.to_le_bytes());
        }
    }

    /// `op rax, rcx` para el grupo 1 (`ext` como en `emit_alu`): la forma
    /// `op r/m64, r64` con `rax` de r/m y `rcx` de reg.
    pub(super) fn emit_alu_rax_rcx(&mut self, ext: u8) {
        let op = match ext {
            0 => 0x01, // add
            1 => 0x09, // or
            4 => 0x21, // and
            5 => 0x29, // sub
            6 => 0x31, // xor
            7 => 0x39, // cmp
            otro => unreachable!("grupo 1 /{otro} no tiene forma con registro aqui"),
        };
        self.code.extend_from_slice(&[0x48, op, 0xC8]);
    }

    /// `op rax, rN` con `rN` de la matriz: como la de arriba con REX.R.
    pub(super) fn emit_alu_rax_rn(&mut self, ext: u8, r: u8) {
        let op = match ext {
            0 => 0x01,
            1 => 0x09,
            4 => 0x21,
            5 => 0x29,
            6 => 0x31,
            7 => 0x39,
            otro => unreachable!("grupo 1 /{otro} no tiene forma con registro aqui"),
        };
        self.code.extend_from_slice(&[0x4C, op, 0xC0 | ((r - 8) << 3)]);
    }
}
