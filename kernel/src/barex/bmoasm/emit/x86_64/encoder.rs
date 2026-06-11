//! Encoder x86-64 mínimo. Sólo lo necesario para BMO simple.

extern crate alloc;
use alloc::vec::Vec;

use crate::barex::{BxError, BxResult};
use super::super::super::parser::ast::Ast;
use super::reg::Reg64;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitError {
    UnsupportedExpr = 1,
    UnsupportedStmt = 2,
    UnknownReg      = 3,
    ImmTooLarge     = 4,
}

pub struct Emitter {
    pub bytes: Vec<u8>,
}

impl Emitter {
    pub const fn new() -> Self { Self { bytes: Vec::new() } }

    /// Genera código para el AST completo. Stub estructural.
    pub fn emit_ast(&mut self, _ast: &Ast) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }

    /// `mov reg, imm64` — REX.W (+ REX.B si r8..r15) + 0xB8+r + imm64.
    pub fn mov_reg_imm64(&mut self, reg: Reg64, imm: u64) {
        let mut rex: u8 = 0x48; // REX.W
        if reg.needs_rex() { rex |= 0x01; } // REX.B
        self.bytes.push(rex);
        self.bytes.push(0xB8 | (reg.code() & 0x07));
        self.bytes.extend_from_slice(&imm.to_le_bytes());
    }

    /// `mov dst, src` — REX.W + 0x89 + ModRM(11 | src << 3 | dst).
    pub fn mov_reg_reg(&mut self, dst: Reg64, src: Reg64) {
        let mut rex = 0x48; // REX.W
        if src.needs_rex() { rex |= 0x04; } // REX.R
        if dst.needs_rex() { rex |= 0x01; } // REX.B
        self.bytes.push(rex);
        self.bytes.push(0x89);
        let modrm = 0xC0 | ((src.code() & 0x07) << 3) | (dst.code() & 0x07);
        self.bytes.push(modrm);
    }

    /// Escribe un placeholder `lea reg, [rip + 0]` y devuelve el offset del disp32.
    pub fn lea_reg_rip_placeholder(&mut self, reg: Reg64) -> usize {
        let mut rex = 0x48; // REX.W
        if reg.needs_rex() { rex |= 0x04; } // REX.R
        self.bytes.push(rex);
        self.bytes.push(0x8D); // LEA
        let modrm = 0x05 | ((reg.code() & 0x07) << 3); // ModRM: [rip + disp32]
        self.bytes.push(modrm);
        let disp_offset = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0, 0, 0]); // Placeholder
        disp_offset
    }

    /// `ret` — 0xC3.
    #[inline(always)]
    pub fn ret(&mut self) { self.bytes.push(0xC3); }

    /// `syscall` — 0x0F 0x05.
    #[inline(always)]
    pub fn syscall(&mut self) { self.bytes.extend_from_slice(&[0x0F, 0x05]); }

    /// `nop` — 0x90.
    #[inline(always)]
    pub fn nop(&mut self) { self.bytes.push(0x90); }

    /// `emit bytes` literal — copia tal cual.
    #[inline(always)]
    pub fn emit_raw(&mut self, raw: &[u8]) { self.bytes.extend_from_slice(raw); }

    /// Devuelve el offset actual (para back-patching de jumps en futuro).
    #[inline(always)]
    pub fn here(&self) -> usize { self.bytes.len() }

    /// Parchea el displacement de la instrucción `lea reg, [rip + disp32]`
    pub fn patch_string_ref(&mut self, disp_offset: usize, rodata_offset: usize, final_code_len: usize) {
        let next_pc = disp_offset + 4; // Disp32 es de 4 bytes
        let target_addr = final_code_len + rodata_offset;
        let disp = (target_addr as isize) - (next_pc as isize);
        let disp32 = (disp as i32) as u32;

        let le_bytes = disp32.to_le_bytes();
        self.bytes[disp_offset] = le_bytes[0];
        self.bytes[disp_offset + 1] = le_bytes[1];
        self.bytes[disp_offset + 2] = le_bytes[2];
        self.bytes[disp_offset + 3] = le_bytes[3];
    }
}

impl Default for Emitter {
    fn default() -> Self { Self::new() }
}
