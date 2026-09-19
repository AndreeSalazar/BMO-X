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
//!
//! # Y el IZQUIERDO en la matriz (la segunda pasada, la misma noche)
//!
//! Con la pila en el 9 %, el 31 % era `mov` entre registros, y el primero de
//! todos `mov rax, r12`: leer una variable de la matriz para operar con ella.
//! Cuando el resultado tiene que ir a `rax` el `mov` sobra igual, porque
//! x86-64 tiene instrucciones de TRES operandos para justo estos casos:
//!
//! ```text
//!    i + 5      lea rax, [r12 + 5]        en vez de  mov rax, r12 ; add rax, 5
//!    i + j      lea rax, [r12 + r13]
//!    i * 7      imul rax, r12, 7
//!    i < n      cmp r12, rcx ; jge        (no hay resultado que mover)
//! ```
//!
//! `emit_lea` es el codificador general de `lea dst, [base + idx*escala +
//! disp]`, y lo usa tambien la direccion de `t[i]`: `lea rax, [rdx + rax*8]`
//! en vez de desplazar y sumar. Los registros van por su numero de 0 a 15,
//! y el REX se compone de los bits altos: es la unica forma de que r12 y r13
//! --los dos con reglas propias en el ModRM-- salgan bien sin una tabla.

use crate::ast::{Expr, TypeSpec};

use super::decidir::inmediato::{inmediato_de, Inmediato};
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

    /// **`lea dst, [base + idx*escala + disp]`**, con los registros por su
    /// numero (0 = rax, 1 = rcx, 2 = rdx, 12..15 = la matriz). `escala` es 1,
    /// 2, 4 u 8; `idx = None` es sin indice.
    ///
    /// Las dos manias del ModRM, resueltas aqui y en ningun otro sitio:
    /// `rsp`/`r12` como base OBLIGAN a un SIB, y `rbp`/`r13` como base con
    /// `mod = 00` significan "sin base" -- por eso con ellas se emite `disp8 = 0`.
    pub(super) fn emit_lea(&mut self, dst: u8, base: u8, idx: Option<u8>, escala: u32, disp: i32) {
        debug_assert!(matches!(escala, 1 | 2 | 4 | 8));
        let rex = 0x48 | ((dst >> 3) << 2) | (idx.map_or(0, |i| (i >> 3) << 1)) | (base >> 3);
        let b = base & 7;
        let modo: u8 = if disp == 0 && b != 5 {
            0
        } else if (-128..=127).contains(&disp) {
            1
        } else {
            2
        };
        let con_sib = idx.is_some() || b == 4;
        let modrm = (modo << 6) | ((dst & 7) << 3) | if con_sib { 4 } else { b };
        self.code.extend_from_slice(&[rex, 0x8D, modrm]);
        if con_sib {
            let ss = escala.trailing_zeros() as u8;
            let i = idx.map_or(4, |i| i & 7);
            self.code.push((ss << 6) | (i << 3) | b);
        }
        match modo {
            1 => self.code.push(disp as u8),
            2 => self.code.extend_from_slice(&disp.to_le_bytes()),
            _ => {}
        }
    }

    /// `imul rax, rN, imm` -- el producto de tres operandos.
    pub(super) fn emit_imul_rax_rn_imm(&mut self, r: u8, imm: Inmediato) {
        let modrm = 0xC0 | (r - 8);
        match imm {
            Inmediato::Corto(c) => self.code.extend_from_slice(&[0x49, 0x6B, modrm, c as u8]),
            Inmediato::Largo(l) => {
                self.code.extend_from_slice(&[0x49, 0x69, modrm]);
                self.code.extend_from_slice(&l.to_le_bytes());
            }
        }
    }

    /// `cmp rN, imm` -- sin pasar por rax: una comparacion no deja resultado.
    pub(super) fn emit_cmp_rn_imm(&mut self, r: u8, imm: Inmediato) {
        let modrm = 0xF8 | (r - 8);
        match imm {
            Inmediato::Corto(c) => self.code.extend_from_slice(&[0x49, 0x83, modrm, c as u8]),
            Inmediato::Largo(l) => {
                self.code.extend_from_slice(&[0x49, 0x81, modrm]);
                self.code.extend_from_slice(&l.to_le_bytes());
            }
        }
    }

    /// `cmp rN, reg` con `reg` por su numero (1 = rcx, 12..15 = la matriz).
    pub(super) fn emit_cmp_rn_reg(&mut self, r: u8, reg: u8) {
        let rex = 0x49 | ((reg >> 3) << 2);
        self.code.extend_from_slice(&[rex, 0x39, 0xC0 | ((reg & 7) << 3) | (r - 8)]);
    }

    /// `mov rcx, imm32` (extendido con signo).
    pub(super) fn emit_mov_rcx_imm(&mut self, imm: Inmediato) {
        let v = match imm {
            Inmediato::Corto(c) => c as i32,
            Inmediato::Largo(l) => l,
        };
        self.code.extend_from_slice(&[0x48, 0xC7, 0xC1]);
        self.code.extend_from_slice(&v.to_le_bytes());
    }

    /// **El operando derecho a rcx, como sea**: inmediato, variable, o -- si
    /// es cualquier otra cosa -- por la pila, guardando `rax`. Lo usan la
    /// division y el desplazamiento por variable, que necesitan el derecho
    /// en `rcx` y el izquierdo en `rax`.
    pub(super) fn emit_derecho_en_rcx(&mut self, b: &Expr) {
        if let Some(imm) = inmediato_de(b) {
            self.emit_mov_rcx_imm(imm);
            return;
        }
        if let Expr::Var(n) = b {
            if self.sabe_cargar(n) {
                self.emit_cargar_en(n, Destino::Rcx);
                return;
            }
        }
        self.code.push(0x50); // push rax (izquierdo)
        self.emit_expr(b);
        self.code.extend_from_slice(&[0x48, 0x89, 0xC1]); // mov rcx, rax
        self.code.push(0x58); // pop rax
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
