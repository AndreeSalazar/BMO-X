//! Encoder x86-64 mínimo. Sólo lo necesario para BMO simple.
//!
//! Tres operaciones esenciales en sesión 15:
//!   - `mov reg64, imm64`    → REX.W + B8+r + imm64
//!   - `ret`                 → 0xC3
//!   - `emit bytes`          → bytes literales
//!
//! Aritmética, jumps y call lo agregará una sesión dedicada al codegen.

extern crate alloc;
use alloc::vec::Vec;

use crate::barex::{BxError, BxResult};
use super::super::parser::ast::Ast;
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

    // ── Encoders básicos ya operativos (útiles para tests del scaffold)

    /// `mov reg, imm64` — REX.W (+ REX.B si r8..r15) + 0xB8+r + imm64.
    pub fn mov_reg_imm64(&mut self, reg: Reg64, imm: u64) {
        let mut rex: u8 = 0x48; // REX.W
        if reg.needs_rex() { rex |= 0x01; } // REX.B
        self.bytes.push(rex);
        self.bytes.push(0xB8 | (reg.code() & 0x07));
        self.bytes.extend_from_slice(&imm.to_le_bytes());
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
}

impl Default for Emitter {
    fn default() -> Self { Self::new() }
}
