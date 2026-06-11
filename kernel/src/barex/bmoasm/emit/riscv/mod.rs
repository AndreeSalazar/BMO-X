//! Backend RISC-V — Futuro soporte para FastOS en RISC-V 64-bit (RV64GC).
//! Provee definiciones de registros y esqueleto del emisor.

#![allow(dead_code)]

use crate::barex::BxResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegRiscv {
    Zero = 0, Ra = 1, Sp = 2, Gp = 3,
    A0 = 10, A1 = 11, A2 = 12, A3 = 13,
}

pub struct EmitterRiscv {
    pub bytes: alloc::vec::Vec<u8>,
}

impl EmitterRiscv {
    pub const fn new() -> Self {
        Self { bytes: alloc::vec::Vec::new() }
    }

    /// `li a0, imm` (Load Immediate) para RV64
    pub fn mov_reg_imm64(&mut self, reg: RegRiscv, imm: u64) {
        // En RISC-V estándar las instrucciones miden 32 bits.
        // Stub para codificar una carga de inmediato de RV64.
        let val_u32 = (imm & 0xFFF) as u32;
        let inst = 0x00000213 | (((reg as u32) & 0x1F) << 7) | (val_u32 << 20);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    /// `ret` — JALR x0, ra, 0 (0x00008067)
    pub fn ret(&mut self) {
        self.bytes.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]);
    }
}
