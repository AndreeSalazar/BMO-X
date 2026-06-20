//! Backend RISC-V — implementación completa de `CodegenBackend`.
//! RV64GC: 32 GPR, loads/stores con offset inmediato, branches condicionales.

use super::super::backend::CodegenBackend;
use super::{RegRiscv, EmitterRiscv};

impl CodegenBackend for EmitterRiscv {
    fn emit_bytes(&mut self, bytes: &[u8]) { self.emit_raw(bytes); }
    fn here(&self) -> usize { self.here() }
    fn bytes_mut(&mut self) -> &mut alloc::vec::Vec<u8> { &mut self.bytes }

    fn mov_acc_imm(&mut self, imm: u64) {
        self.li(10, imm);
    }

    fn mov_acc_reg(&mut self, src: u32) {
        if src != 10 { self.mv(10, src); }
    }

    fn mov_reg_acc(&mut self, dst: u32) {
        if dst != 10 { self.mv(dst, 10); }
    }

    fn mov_reg_reg(&mut self, dst: u32, src: u32) {
        if dst != src { self.mv(dst, src); }
    }

    fn load_var(&mut self, frame_offset: i32) {
        // LD a0, offset(fp)  — fp = x8
        let offset = (frame_offset as u32) & 0xFFF;
        let inst: u32 = 0x00003003 | (offset << 20) | (8 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn store_var(&mut self, frame_offset: i32) {
        // SD a0, offset(fp)
        let offset = (frame_offset as u32) as i32;
        let imm11_5 = ((offset >> 5) as u32) & 0x7F;
        let imm4_0 = (offset as u32) & 0x1F;
        let inst: u32 = 0x00003023 | (imm11_5 << 25) | (10 << 20) | (8 << 15) | (0x03 << 12) | imm4_0;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn push_acc(&mut self) {}
    fn pop_acc(&mut self) {}

    fn add_acc(&mut self, reg: u32) {
        // ADD a0, a0, rd
        let inst: u32 = 0x00000033 | (reg << 20) | (10 << 15) | (0x0 << 12) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn sub_acc(&mut self, reg: u32) {
        // SUB a0, a0, rd
        let inst: u32 = 0x40000033 | (reg << 20) | (10 << 15) | (0x0 << 12) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn mul_acc(&mut self, reg: u32) {
        // MUL a0, a0, rd (M extension: funct7=0x01, funct3=0)
        let inst: u32 = 0x02000033 | (reg << 20) | (10 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn div_acc(&mut self, reg: u32) {
        // DIV a0, a0, rd
        let inst: u32 = 0x02004033 | (reg << 20) | (10 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn mod_acc(&mut self, reg: u32) {
        // REM a0, a0, rd
        let inst: u32 = 0x02006033 | (reg << 20) | (10 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn and_acc(&mut self, reg: u32) {
        let inst: u32 = 0x00007033 | (reg << 20) | (10 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn or_acc(&mut self, reg: u32) {
        let inst: u32 = 0x00006033 | (reg << 20) | (10 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn xor_acc(&mut self, reg: u32) {
        let inst: u32 = 0x00004033 | (reg << 20) | (10 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn shl_acc(&mut self, reg: u32) {
        // SLL a0, a0, rd (funct3=0x1)
        let inst: u32 = 0x00001033 | (reg << 20) | (10 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn shr_acc(&mut self, reg: u32) {
        // SRL a0, a0, rd (funct3=0x5)
        let inst: u32 = 0x00005033 | (reg << 20) | (10 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn zero_acc(&mut self) {
        self.mv(10, 0);
    }

    fn cmp_eq_acc(&mut self, reg: u32) {
        // XOR a0, a0, rd; SLTU a0, x0, a0 → a0 = (a0 != 0) ? 1 : 0; then NOT
        // Simpler: SUB a0, a0, rd; SLTU a0, x0, a0 → (a0 == rd) ? 0 : 1; then XORI
        // Actually: XOR a0, rd, a0; SLTU a1, x0, a0; MV a0, a1
        self.xor_acc(reg); // a0 = a0 ^ reg (0 if equal)
        self.sltu_from_zero(); // a0 = (a0 != 0) ? 1 : 0
    }

    fn cmp_gt_acc(&mut self, reg: u32) {
        // SLT a0, a0, rd → a0 = (a0 < rd) ? 1 : 0 = (rd > a0) ? 1 : 0
        let inst: u32 = 0x00002033 | (reg << 20) | (10 << 15) | (0x0 << 12) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn cmp_lt_acc(&mut self, reg: u32) {
        // SLT a0, rd, a0 → a0 = (rd < a0) ? 1 : 0
        let inst: u32 = 0x00002033 | (10 << 20) | (reg << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn sete_acc(&mut self) {
        self.sltu_from_zero();
    }

    fn setg_acc(&mut self) {
        self.sltu_from_zero();
    }

    fn setl_acc(&mut self) {
        self.sltu_from_zero();
    }

    fn test_acc(&mut self) {
        // SLTU a0, x0, a0 → a0 = (a0 != 0) ? 1 : 0
        self.sltu_from_zero();
    }

    fn je_rel32(&mut self) -> usize {
        // BEQ a0, x0, offset (branch if a0 == 0)
        let off = self.bytes.len();
        let inst: u32 = 0x00000063; // BEQ x0, x0, 0 — placeholder
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        off
    }

    fn jne_rel32(&mut self) -> usize {
        // BNE a0, x0, offset
        let off = self.bytes.len();
        let inst: u32 = 0x00001063;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        off
    }

    fn jmp_rel32(&mut self) -> usize {
        // JAL x0, offset (unconditional)
        let off = self.bytes.len();
        let inst: u32 = 0x0000006F;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        off
    }

    fn nop(&mut self) { self.nop(); }
    fn syscall_inst(&mut self) { self.syscall(); }
    fn ret(&mut self) { self.ret(); }

    fn call_rel32(&mut self) -> usize {
        // JAL ra, offset
        let off = self.bytes.len();
        let inst: u32 = 0x000000EF; // JAL x1, 0
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        off
    }

    fn patch_string_ref(&mut self, d: usize, r: usize, f: usize) {
        self.patch_string_ref(d, r, f);
    }

    fn patch_rel32(&mut self, offset: usize, from: usize, to: usize) {
        let byte_offset = (to as isize) - (from as isize);
        let inst = u32::from_le_bytes([
            self.bytes[offset], self.bytes[offset+1],
            self.bytes[offset+2], self.bytes[offset+3],
        ]);
        let opcode = inst & 0x7F;
        let patched = match opcode {
            0x63 => {
                // B-type: BEQ/BNE
                let imm = (byte_offset as i32) as u32;
                let b12 = ((imm >> 12) & 0x1) << 31;
                let b11 = ((imm >> 11) & 0x1) << 7;
                let b10_5 = ((imm >> 5) & 0x3F) << 25;
                let b4_1 = ((imm >> 1) & 0xF) << 8;
                (inst & 0x1FFF) | b12 | b11 | b10_5 | b4_1
            }
            0x6F => {
                // J-type: JAL
                let imm = (byte_offset as i32) as u32;
                let b20 = ((imm >> 20) & 0x1) << 31;
                let b10_1 = ((imm >> 1) & 0x3FF) << 21;
                let b11 = ((imm >> 11) & 0x1) << 20;
                let b19_12 = ((imm >> 12) & 0xFF) << 12;
                (inst & 0xFFF) | b20 | b19_12 | b11 | b10_1
            }
            _ => inst,
        };
        let le = patched.to_le_bytes();
        self.bytes[offset..offset+4].copy_from_slice(&le);
    }

    fn emit_prologue(&mut self) -> usize {
        // ADDI sp, sp, -16 (placeholder)
        let off = self.bytes.len();
        let inst: u32 = 0xFF010113;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        // SD ra, 8(sp)
        let inst2: u32 = 0x00813023;
        self.bytes.extend_from_slice(&inst2.to_le_bytes());
        // SD fp, 0(sp)
        let inst3: u32 = 0x00113423;
        self.bytes.extend_from_slice(&inst3.to_le_bytes());
        // ADDI fp, sp, 16
        let inst4: u32 = 0x01010113;
        self.bytes.extend_from_slice(&inst4.to_le_bytes());
        off
    }

    fn patch_frame_size(&mut self, prologue_offset: usize, frame_size: u32) {
        let aligned = (frame_size + 15) & !15;
        let imm = (-(aligned as i32)) as u32;
        let inst: u32 = 0x00000113 | ((imm & 0xFFF) << 20);
        let le = inst.to_le_bytes();
        self.bytes[prologue_offset..prologue_offset+4].copy_from_slice(&le);
    }

    fn emit_epilogue(&mut self) {
        // LD ra, 8(sp)
        let inst: u32 = 0x00813083;
        self.bytes.extend_from_slice(&inst.to_le_bytes());
        // LD fp, 0(sp)
        let inst2: u32 = 0x00013403;
        self.bytes.extend_from_slice(&inst2.to_le_bytes());
        // ADDI sp, sp, 16
        let inst3: u32 = 0x01010113;
        self.bytes.extend_from_slice(&inst3.to_le_bytes());
        self.ret();
    }

    fn arg_reg_count(&self) -> usize { 8 }
    fn arg_reg(&self, i: usize) -> Option<u32> {
        if i < 8 { Some(10 + i as u32) } else { None } // a0-a7
    }
    fn ret_reg(&self) -> u32 { 10 }  // a0
    fn acc_reg(&self) -> u32 { 10 }   // a0
    fn scratch_reg(&self) -> u32 { 5 } // t0

    fn parse_reg(&self, name: &str) -> Option<u32> {
        match name {
            "zero" => Some(0), "ra" | "x1" => Some(1),
            "sp" | "x2" => Some(2), "gp" | "x3" => Some(3),
            "tp" | "x4" => Some(4),
            "t0" | "x5" => Some(5), "t1" | "x6" => Some(6), "t2" | "x7" => Some(7),
            "s0" | "fp" | "x8" => Some(8), "s1" | "x9" => Some(9),
            "a0" | "x10" => Some(10), "a1" | "x11" => Some(11),
            "a2" | "x12" => Some(12), "a3" | "x13" => Some(13),
            "a4" | "x14" => Some(14), "a5" | "x15" => Some(15),
            "a6" | "x16" => Some(16), "a7" | "x17" => Some(17),
            "s2" | "x18" => Some(18), "s3" | "x19" => Some(19),
            "s4" | "x20" => Some(20), "s5" | "x21" => Some(21),
            "s6" | "x22" => Some(22), "s7" | "x23" => Some(23),
            "s8" | "x24" => Some(24), "s9" | "x25" => Some(25),
            "s10" | "x26" => Some(26), "s11" | "x27" => Some(27),
            "t3" | "x28" => Some(28), "t4" | "x29" => Some(29),
            "t5" | "x30" => Some(30), "t6" | "x31" => Some(31),
            _ => None,
        }
    }

    fn intrinsic_bytes(&self, name: &str) -> Option<&'static [u8]> {
        match name {
            "syscall" | "ecall" => Some(&[0x73, 0x00, 0x00, 0x00]),
            "nop" => Some(&[0x13, 0x00, 0x00, 0x00]),
            "pausa" => Some(&[0x0F, 0x00, 0x00, 0x00]),  // fence
            "int3"  => Some(&[0x73, 0x00, 0x10, 0x00]),  // ebreak
            "hlt"   => Some(&[0x73, 0x00, 0x00, 0x00]),  // ecall (similar)
            "rdtsc" => Some(&[0x73, 0x00, 0x00, 0x00]),  // time csr read (placeholder)
            "lfence" => Some(&[0x0F, 0x00, 0xF0, 0x0F]), // fence i, iorw
            "mfence" => Some(&[0x0F, 0x00, 0xF0, 0xFF]), // fence iorw, iorw
            "sfence" => Some(&[0x0F, 0x00, 0x0F, 0xF0]), // fence w, w
            _ => None,
        }
    }
}

impl EmitterRiscv {
    fn r(&self, reg: u32) -> RegRiscv {
        match reg {
            0 => RegRiscv::Zero, 1 => RegRiscv::Ra, 2 => RegRiscv::Sp,
            3 => RegRiscv::Gp, 10 => RegRiscv::A0, 11 => RegRiscv::A1,
            12 => RegRiscv::A2, 13 => RegRiscv::A3,
            _ => RegRiscv::A0,
        }
    }

    fn li(&mut self, reg: u32, imm: u64) {
        if imm <= 0xFFF {
            // ADDI rd, x0, imm
            let inst: u32 = 0x00000013 | ((reg & 0x1F) << 7) | (((imm as u32) & 0xFFF) << 20);
            self.bytes.extend_from_slice(&inst.to_le_bytes());
        } else {
            // LUI + ADDI for larger values
            let hi = ((imm + 0x800) >> 12) as u32;
            let lo = (imm as u32).wrapping_sub(hi << 12) & 0xFFF;
            let lui: u32 = 0x00000037 | ((reg & 0x1F) << 7) | (hi << 12);
            self.bytes.extend_from_slice(&lui.to_le_bytes());
            if lo != 0 {
                let addi: u32 = 0x00000013 | ((reg & 0x1F) << 7) | ((reg & 0x1F) << 15) | (lo << 20);
                self.bytes.extend_from_slice(&addi.to_le_bytes());
            }
        }
    }

    fn mv(&mut self, dst: u32, src: u32) {
        // ADDI rd, rs, 0
        let inst: u32 = 0x00000013 | ((dst & 0x1F) << 7) | ((src & 0x1F) << 15);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }

    fn sltu_from_zero(&mut self) {
        // SLTU a0, x0, a0 → a0 = (x0 < a0) ? 1 : 0 = (a0 != 0) ? 1 : 0
        let inst: u32 = 0x00003033 | (10 << 20) | (0 << 15) | (10 << 7);
        self.bytes.extend_from_slice(&inst.to_le_bytes());
    }
}
