//! Backend RISC-V — Futuro soporte para FastOS en RISC-V 64-bit (RV64GC).
//! Provee definiciones de registros y esqueleto del emisor.

#![allow(dead_code)]

pub mod backend_impl;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegRiscv {
    Zero = 0, Ra = 1, Sp = 2, Gp = 3,
    A0 = 10, A1 = 11, A2 = 12, A3 = 13,
}

impl RegRiscv {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "zero" => Some(Self::Zero), "ra" => Some(Self::Ra),
            "sp" => Some(Self::Sp), "gp" => Some(Self::Gp),
            "a0" => Some(Self::A0), "a1" => Some(Self::A1),
            "a2" => Some(Self::A2), "a3" => Some(Self::A3),
            _ => None,
        }
    }
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
        let val_u32 = (imm & 0xFFF) as u32;
        let inst = 0x00000213 | (((reg as u32) & 0x1F) << 7) | (val_u32 << 20);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    /// `mv dst, src` -> `addi dst, src, 0`
    pub fn mov_reg_reg(&mut self, dst: RegRiscv, src: RegRiscv) {
        let inst = 0x00000013 | (((dst as u32) & 0x1F) << 7) | (((src as u32) & 0x1F) << 15);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    /// LEA placeholder con AUIPC + ADDI
    pub fn lea_reg_rip_placeholder(&mut self, reg: RegRiscv) -> usize {
        let disp_offset = self.bytes.len();
        // AUIPC reg, 0
        let inst_auipc = 0x00000017 | (((reg as u32) & 0x1F) << 7);
        // ADDI reg, reg, 0
        let inst_addi = 0x00000013 | (((reg as u32) & 0x1F) << 7) | (((reg as u32) & 0x1F) << 15);
        
        self.bytes.extend_from_slice(&inst_auipc.to_le_bytes());
        self.bytes.extend_from_slice(&inst_addi.to_le_bytes());
        
        disp_offset
    }

    /// `ret` — JALR x0, ra, 0 (0x00008067)
    pub fn ret(&mut self) {
        self.bytes.extend_from_slice(&[0x67, 0x80, 0x00, 0x00]);
    }

    /// `syscall` -> `ecall`
    pub fn syscall(&mut self) {
        self.bytes.extend_from_slice(&[0x73, 0x00, 0x00, 0x00]);
    }

    /// `nop` -> `addi x0, x0, 0`
    pub fn nop(&mut self) {
        self.bytes.extend_from_slice(&[0x13, 0x00, 0x00, 0x00]);
    }

    pub fn emit_raw(&mut self, raw: &[u8]) {
        self.bytes.extend_from_slice(raw);
    }

    pub fn here(&self) -> usize {
        self.bytes.len()
    }

    /// Patch AUIPC + ADDI for PC-relative label
    pub fn patch_string_ref(&mut self, disp_offset: usize, rodata_offset: usize, final_code_len: usize) {
        let target_addr = final_code_len + rodata_offset;
        let pc = disp_offset;
        let offset = (target_addr as isize) - (pc as isize);
        
        let hi = (offset + 0x800) as u32 & 0xFFFFF000;
        let lo = (offset as u32) & 0xFFF;
        
        let mut inst_auipc = u32::from_le_bytes([
            self.bytes[disp_offset],
            self.bytes[disp_offset + 1],
            self.bytes[disp_offset + 2],
            self.bytes[disp_offset + 3],
        ]);
        let mut inst_addi = u32::from_le_bytes([
            self.bytes[disp_offset + 4],
            self.bytes[disp_offset + 5],
            self.bytes[disp_offset + 6],
            self.bytes[disp_offset + 7],
        ]);
        
        inst_auipc &= 0x00000FFF;
        inst_auipc |= hi;
        
        inst_addi &= 0x000FFFFF;
        inst_addi |= lo << 20;
        
        let le_auipc = inst_auipc.to_le_bytes();
        let le_addi = inst_addi.to_le_bytes();
        
        self.bytes[disp_offset] = le_auipc[0];
        self.bytes[disp_offset + 1] = le_auipc[1];
        self.bytes[disp_offset + 2] = le_auipc[2];
        self.bytes[disp_offset + 3] = le_auipc[3];
        
        self.bytes[disp_offset + 4] = le_addi[0];
        self.bytes[disp_offset + 5] = le_addi[1];
        self.bytes[disp_offset + 6] = le_addi[2];
        self.bytes[disp_offset + 7] = le_addi[3];
    }
}

impl Default for EmitterRiscv {
    fn default() -> Self {
        Self::new()
    }
}
