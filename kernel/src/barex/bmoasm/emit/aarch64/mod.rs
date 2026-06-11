//! Backend AArch64 (ARM64) — Futuro soporte para FastOS en ARMv8+.
//! Provee definiciones de registros y esqueleto del emisor.

#![allow(dead_code)]

use crate::barex::BxResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegArm {
    X0 = 0, X1 = 1, X2 = 2, X3 = 3,
    X4 = 4, X5 = 5, X6 = 6, X7 = 7,
    X30 = 30, // Link Register
    SP = 31,
}

pub struct EmitterArm {
    pub bytes: alloc::vec::Vec<u8>,
}

impl EmitterArm {
    pub const fn new() -> Self {
        Self { bytes: alloc::vec::Vec::new() }
    }

    /// `mov x0, #imm` literal (simplificado para AArch64)
    pub fn mov_reg_imm64(&mut self, reg: RegArm, imm: u64) {
        // En ARM64, una instrucción mide 32 bits (4 bytes).
        // Stub para codificar un MOVz / MOVk de AArch64.
        let val_u32 = (imm & 0xFFFFFFFF) as u32;
        let inst = 0xD2800000 | ((reg as u32) & 0x1F) | ((val_u32 & 0xFFFF) << 5);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    /// `ret` — opcode `RET` (0xD65F03C0).
    pub fn ret(&mut self) {
        self.bytes.extend_from_slice(&[0xC0, 0x03, 0x5F, 0xD6]);
    }
}
