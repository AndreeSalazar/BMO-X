//! Backend AArch64 — implementación completa de `CodegenBackend`.
//! ARMv8-A 64-bit: 31 GPR, loads/stores con offset, B.cond branches.

use super::super::backend::CodegenBackend;
use super::{RegArm, EmitterArm};

impl CodegenBackend for EmitterArm {
    fn emit_bytes(&mut self, bytes: &[u8]) { self.emit_raw(bytes); }
    fn here(&self) -> usize { self.here() }
    fn bytes_mut(&mut self) -> &mut alloc::vec::Vec<u8> { &mut self.bytes }

    fn mov_acc_imm(&mut self, imm: u64) {
        self.mov_reg_imm64(RegArm::X0, imm);
    }

    fn mov_acc_reg(&mut self, src: u32) {
        if src != 0 { self.mov_reg_reg(RegArm::X0, self.r(src)); }
    }

    fn mov_reg_acc(&mut self, dst: u32) {
        if dst != 0 { self.mov_reg_reg(self.r(dst), RegArm::X0); }
    }

    fn mov_reg_reg(&mut self, dst: u32, src: u32) {
        if dst != src {
            self.mov_reg_reg(self.r(dst), self.r(src));
        }
    }

    fn load_var(&mut self, frame_offset: i32) {
        let imm12 = ((frame_offset as u32) / 8) as u32;
        let inst: u32 = 0xF9400000 | (imm12 << 10) | (29 << 5) | 0;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn store_var(&mut self, frame_offset: i32) {
        let imm12 = ((frame_offset as u32) / 8) as u32;
        let inst: u32 = 0xF9000000 | (imm12 << 10) | (29 << 5) | 0;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn push_acc(&mut self) {}
    fn pop_acc(&mut self) {}

    fn add_acc(&mut self, reg: u32) { self.emit_alu_reg(0x8Bu32, reg); }
    fn sub_acc(&mut self, reg: u32) { self.emit_alu_reg(0xCBu32, reg); }

    fn mul_acc(&mut self, reg: u32) {
        let inst: u32 = 0x9B007C00 | ((reg & 0x1F) << 16);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn div_acc(&mut self, reg: u32) {
        let inst: u32 = 0x9AC00C00 | ((reg & 0x1F) << 16);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn mod_acc(&mut self, reg: u32) {
        // SDIV x2, x0, xR
        let div_inst: u32 = 0x9AC00C02 | ((reg & 0x1F) << 16);
        self.bytes.extend_from_slice(&div_inst.to_le_bytes());
        // MSUB x0, x2, xR, x0 → x0 = x0 - (x0/xR)*xR
        let msub_inst: u32 = 0x9B008C00 | ((reg & 0x1F) << 16) | (2 << 10);
        self.bytes.extend_from_slice(&msub_inst.to_le_bytes());
    }

    fn and_acc(&mut self, reg: u32) {
        let inst: u32 = 0x8A000000 | ((reg & 0x1F) << 16) | 0;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn or_acc(&mut self, reg: u32) {
        let inst: u32 = 0xAA000000 | ((reg & 0x1F) << 16) | 0;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn xor_acc(&mut self, reg: u32) {
        let inst: u32 = 0xCA000000 | ((reg & 0x1F) << 16) | 0;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn shl_acc(&mut self, reg: u32) {
        let inst: u32 = 0x9AC02000 | ((reg & 0x1F) << 16);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn shr_acc(&mut self, reg: u32) {
        let inst: u32 = 0x9AC02400 | ((reg & 0x1F) << 16);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn zero_acc(&mut self) {
        let inst: u32 = 0xD2800000;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn cmp_eq_acc(&mut self, reg: u32) {
        self.emit_cmp(reg);
        self.emit_cset(0x0);
    }

    fn cmp_gt_acc(&mut self, reg: u32) {
        // SUBS xzr, xR, x0 → reg > acc
        let inst: u32 = 0xEB00001F | ((reg & 0x1F) << 16) | (0 << 5);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        self.emit_cset(0xC);
    }

    fn cmp_lt_acc(&mut self, reg: u32) {
        // SUBS xzr, xR, x0 → reg < acc
        let inst: u32 = 0xEB00001F | ((reg & 0x1F) << 16) | (0 << 5);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        self.emit_cset(0xB);
    }

    fn sete_acc(&mut self) { self.emit_cset(0x0); }
    fn setg_acc(&mut self) { self.emit_cset(0xC); }
    fn setl_acc(&mut self) { self.emit_cset(0xB); }

    fn test_acc(&mut self) {
        let inst: u32 = 0xF100001F;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn je_rel32(&mut self) -> usize {
        let inst: u32 = 0x54000000;
        let off = self.bytes.len();
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        off
    }

    fn jne_rel32(&mut self) -> usize {
        let inst: u32 = 0x54000001;
        let off = self.bytes.len();
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        off
    }

    fn jmp_rel32(&mut self) -> usize {
        let inst: u32 = 0x14000000;
        let off = self.bytes.len();
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        off
    }

    fn nop(&mut self) { self.nop(); }
    fn syscall_inst(&mut self) { self.syscall(); }
    fn ret(&mut self) { self.ret(); }

    fn call_rel32(&mut self) -> usize {
        let inst: u32 = 0x94000000;
        let off = self.bytes.len();
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        off
    }

    fn patch_string_ref(&mut self, d: usize, r: usize, f: usize) {
        self.patch_string_ref(d, r, f);
    }

    fn patch_rel32(&mut self, offset: usize, from: usize, to: usize) {
        let byte_offset = (to as isize) - (from as isize);
        let inst_offset = (byte_offset / 4) as i32;
        let inst = u32::from_le_bytes([
            self.bytes[offset], self.bytes[offset+1],
            self.bytes[offset+2], self.bytes[offset+3],
        ]);
        let kind = (inst >> 26) & 0x3F;
        let patched = match kind {
            0x05 => (inst & 0xFC000000) | ((inst_offset as u32) & 0x03FFFFFF),       // B
            0x25 => (inst & 0xFF00001F) | (((inst_offset as u32) & 0x7FFFF) << 5),    // B.cond
            0x26 => (inst & 0xFC000000) | ((inst_offset as u32) & 0x03FFFFFF),        // BL
            _ => inst,
        };
        let le = patched.to_le_bytes();
        self.bytes[offset..offset+4].copy_from_slice(&le);
    }

    fn emit_prologue(&mut self) -> usize {
        // STP x29, x30, [sp, #-16]!
        let inst: u32 = 0xA9BF7BFD;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        // MOV x29, sp
        let inst2: u32 = 0x910003FD;
        self.bytes.extend_from_slice(&inst2.to_le_bytes());
        // SUB sp, sp, #0 (placeholder)
        let off = self.bytes.len();
        let inst3: u32 = 0xD10003FF;
        self.bytes.extend_from_slice(&inst3.to_le_bytes());
        off
    }

    fn patch_frame_size(&mut self, prologue_offset: usize, frame_size: u32) {
        let aligned = (frame_size + 15) & !15;
        let imm12 = aligned / 16;
        let inst: u32 = 0xD10003FF | ((imm12 as u32) << 10);
        let le = inst.to_le_bytes();
        self.bytes[prologue_offset..prologue_offset+4].copy_from_slice(&le);
    }

    fn emit_epilogue(&mut self) {
        let inst: u32 = 0xA8C17BFD;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        self.ret();
    }

    fn arg_reg_count(&self) -> usize { 8 }
    fn arg_reg(&self, i: usize) -> Option<u32> {
        if i < 8 { Some(i as u32) } else { None }
    }
    fn ret_reg(&self) -> u32 { 0 }
    fn acc_reg(&self) -> u32 { 0 }
    fn scratch_reg(&self) -> u32 { 1 }

    fn parse_reg(&self, name: &str) -> Option<u32> {
        match name {
            "x0" => Some(0), "x1" => Some(1), "x2" => Some(2), "x3" => Some(3),
            "x4" => Some(4), "x5" => Some(5), "x6" => Some(6), "x7" => Some(7),
            "x8" => Some(8), "x9" => Some(9), "x10" => Some(10), "x11" => Some(11),
            "x12" => Some(12), "x13" => Some(13), "x14" => Some(14), "x15" => Some(15),
            "x16" => Some(16), "x17" => Some(17), "x18" => Some(18), "x19" => Some(19),
            "x20" => Some(20), "x21" => Some(21), "x22" => Some(22), "x23" => Some(23),
            "x24" => Some(24), "x25" => Some(25), "x26" => Some(26), "x27" => Some(27),
            "x28" => Some(28), "x29" | "fp" => Some(29), "x30" | "lr" => Some(30),
            "sp" => Some(31),
            _ => None,
        }
    }

    fn intrinsic_bytes(&self, name: &str) -> Option<&'static [u8]> {
        match name {
            "syscall" => Some(&[0x01, 0x00, 0x00, 0xD4]),
            "nop"     => Some(&[0x1F, 0x20, 0x03, 0xD5]),
            _ => None,
        }
    }
}

impl EmitterArm {
    fn r(&self, reg: u32) -> RegArm {
        match reg {
            0 => RegArm::X0, 1 => RegArm::X1, 2 => RegArm::X2, 3 => RegArm::X3,
            4 => RegArm::X4, 5 => RegArm::X5, 6 => RegArm::X6, 7 => RegArm::X7,
            _ => RegArm::X0,
        }
    }

    fn emit_alu_reg(&mut self, opcode: u32, reg: u32) {
        let inst: u32 = opcode << 24 | ((reg & 0x1F) << 16) | 0;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn emit_cmp(&mut self, reg: u32) {
        let inst: u32 = 0xEB00001F | ((reg & 0x1F) << 16);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn emit_cset(&mut self, cond: u32) {
        let inst: u32 = 0x9A9F07E0 | (cond << 12);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }
}
