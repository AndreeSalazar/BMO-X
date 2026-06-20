//! Backend AArch64 (ARM64) — Futuro soporte para FastOS en ARMv8+.
//! Provee definiciones de registros y esqueleto del emisor.

#![allow(dead_code)]

pub mod backend_impl;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegArm {
    X0 = 0, X1 = 1, X2 = 2, X3 = 3,
    X4 = 4, X5 = 5, X6 = 6, X7 = 7,
    X30 = 30, // Link Register
    SP = 31,
}

impl RegArm {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "x0" => Some(Self::X0), "x1" => Some(Self::X1),
            "x2" => Some(Self::X2), "x3" => Some(Self::X3),
            "x4" => Some(Self::X4), "x5" => Some(Self::X5),
            "x6" => Some(Self::X6), "x7" => Some(Self::X7),
            "x30" => Some(Self::X30), "sp" => Some(Self::SP),
            _ => None,
        }
    }
}

pub struct EmitterArm {
    pub bytes: alloc::vec::Vec<u8>,
}

impl EmitterArm {
    pub const fn new() -> Self {
        Self { bytes: alloc::vec::Vec::new() }
    }

    /// `mov x0, #imm` (simplificado para AArch64)
    pub fn mov_reg_imm64(&mut self, reg: RegArm, imm: u64) {
        let val_u32 = (imm & 0xFFFFFFFF) as u32;
        let inst = 0xD2800000 | ((reg as u32) & 0x1F) | ((val_u32 & 0xFFFF) << 5);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    /// `mov dst, src` -> `orr dst, xzr, src`
    pub fn mov_reg_reg(&mut self, dst: RegArm, src: RegArm) {
        let inst = 0xAA0003E0 | (((src as u32) & 0x1F) << 16) | ((dst as u32) & 0x1F);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    /// LEA placeholder con ADR
    pub fn lea_reg_rip_placeholder(&mut self, reg: RegArm) -> usize {
        let disp_offset = self.bytes.len();
        // ADR reg, #0 (placeholder)
        let inst = 0x30000000 | ((reg as u32) & 0x1F);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        disp_offset
    }

    /// `ret` -> opcode `RET` (0xD65F03C0)
    pub fn ret(&mut self) {
        self.bytes.extend_from_slice(&[0xC0, 0x03, 0x5F, 0xD6]);
    }

    /// `syscall` -> `svc #0`
    pub fn syscall(&mut self) {
        self.bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0xD4]);
    }

    /// `nop` -> `nop` instruction
    pub fn nop(&mut self) {
        self.bytes.extend_from_slice(&[0x1F, 0x20, 0x03, 0xD5]);
    }

    pub fn emit_raw(&mut self, raw: &[u8]) {
        self.bytes.extend_from_slice(raw);
    }

    pub fn here(&self) -> usize {
        self.bytes.len()
    }

    /// Patch ADR instruction for PC-relative label
    pub fn patch_string_ref(&mut self, disp_offset: usize, rodata_offset: usize, final_code_len: usize) {
        let target_addr = final_code_len + rodata_offset;
        let pc = disp_offset;
        let offset = (target_addr as isize) - (pc as isize);
        
        let imm = (offset & 0x1FFFFF) as u32;
        let immlo = imm & 0x3;
        let immhi = (imm >> 2) & 0x7FFFF;
        
        let mut inst = u32::from_le_bytes([
            self.bytes[disp_offset],
            self.bytes[disp_offset + 1],
            self.bytes[disp_offset + 2],
            self.bytes[disp_offset + 3],
        ]);
        
        inst &= !(0x3 << 29);
        inst &= !(0x7FFFF << 5);
        inst |= immlo << 29;
        inst |= immhi << 5;
        
        let le = inst.to_le_bytes();
        self.bytes[disp_offset] = le[0];
        self.bytes[disp_offset + 1] = le[1];
        self.bytes[disp_offset + 2] = le[2];
        self.bytes[disp_offset + 3] = le[3];
    }
}

impl Default for EmitterArm {
    fn default() -> Self {
        Self::new()
    }
}
