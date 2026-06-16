//! Backend x86-64 — implementación completa de `CodegenBackend`.

use super::super::backend::CodegenBackend;
use super::reg::Reg64;
use super::encoder::Emitter;

/// Mapeo de registros genéricos (u32) a `Reg64`.
fn reg(id: u32) -> Reg64 {
    match id {
        0 => Reg64::Rax, 1 => Reg64::Rcx, 2 => Reg64::Rdx, 3 => Reg64::Rbx,
        4 => Reg64::Rsp, 5 => Reg64::Rbp, 6 => Reg64::Rsi, 7 => Reg64::Rdi,
        8 => Reg64::R8,  9 => Reg64::R9,  10 => Reg64::R10, 11 => Reg64::R11,
        12 => Reg64::R12, 13 => Reg64::R13, 14 => Reg64::R14, 15 => Reg64::R15,
        _ => Reg64::Rax,
    }
}

impl CodegenBackend for Emitter {
    fn emit_bytes(&mut self, bytes: &[u8]) { self.emit_raw(bytes); }
    fn here(&self) -> usize { self.here() }
    fn bytes_mut(&mut self) -> &mut alloc::vec::Vec<u8> { &mut self.bytes }

    fn mov_acc_imm(&mut self, imm: u64) { self.mov_reg_imm64(Reg64::Rax, imm); }
    fn mov_acc_reg(&mut self, src: u32) { self.mov_reg_reg(Reg64::Rax, reg(src)); }
    fn mov_reg_acc(&mut self, dst: u32) { self.mov_reg_reg(reg(dst), Reg64::Rax); }
    fn mov_reg_reg(&mut self, dst: u32, src: u32) { self.mov_reg_reg(reg(dst), reg(src)); }

    fn load_var(&mut self, frame_offset: i32) {
        if frame_offset >= -128 && frame_offset <= 127 {
            self.mov_rax_rbp_disp8(frame_offset as i8);
        } else {
            self.mov_rax_rbp_disp32(frame_offset);
        }
    }

    fn store_var(&mut self, frame_offset: i32) {
        if frame_offset >= -128 && frame_offset <= 127 {
            self.mov_rbp_disp8_rax(frame_offset as i8);
        } else {
            self.mov_rbp_disp32_rax(frame_offset);
        }
    }

    fn push_acc(&mut self) { self.push_rax(); }
    fn pop_acc(&mut self) { self.pop_rax(); }
    fn add_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.add_rax_rcx();
    }
    fn sub_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.sub_rax_rcx();
    }
    fn mul_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.imul_rax_rcx();
    }
    fn div_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.idiv_rcx();
    }
    fn mod_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.idiv_rcx();
        // remainder is in RDX — move to RAX
        self.mov_reg_reg(Reg64::Rax, Reg64::Rdx);
    }
    fn and_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.and_rax_rcx();
    }
    fn or_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.or_rax_rcx();
    }
    fn xor_acc(&mut self, r: u32) {
        // xor rax, reg
        let mut rex = 0x48;
        let rr = reg(r);
        if rr.needs_rex() { rex |= 0x04; }
        self.bytes.push(rex);
        self.bytes.push(0x31); // XOR r/m64, r64
        let modrm = 0xC0 | ((rr.code() & 0x07) << 3) | (Reg64::Rax.code() & 0x07);
        self.bytes.push(modrm);
    }
    fn shl_acc(&mut self, r: u32) {
        // shl rax, cl  (cl = rcx)
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.bytes.extend_from_slice(&[0x48, 0xD3, 0xE0]); // shl rax, cl
    }
    fn shr_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.bytes.extend_from_slice(&[0x48, 0xD3, 0xE8]); // shr rax, cl
    }
    fn zero_acc(&mut self) { self.xor_rax_rax(); }

    fn cmp_eq_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.cmp_reg_reg(Reg64::Rax, Reg64::Rcx);
        self.xor_rax_rax();
        self.sete_al();
        self.movzx_rax_al();
    }
    fn cmp_gt_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.cmp_reg_reg(Reg64::Rcx, Reg64::Rax); // cmp rcx, rax → rcx > rax
        self.xor_rax_rax();
        self.bytes.extend_from_slice(&[0x0F, 0x9F, 0xC0]); // setg al
        self.movzx_rax_al();
    }
    fn cmp_lt_acc(&mut self, r: u32) {
        if reg(r) != Reg64::Rcx { self.mov_reg_reg(Reg64::Rcx, reg(r)); }
        self.cmp_reg_reg(Reg64::Rcx, Reg64::Rax); // cmp rcx, rax → rcx < rax
        self.xor_rax_rax();
        self.bytes.extend_from_slice(&[0x0F, 0x9C, 0xC0]); // setl al
        self.movzx_rax_al();
    }
    fn sete_acc(&mut self) { self.sete_al(); self.movzx_rax_al(); }
    fn setg_acc(&mut self) {
        self.bytes.extend_from_slice(&[0x0F, 0x9F, 0xC0]); // setg al
        self.movzx_rax_al();
    }
    fn setl_acc(&mut self) {
        self.bytes.extend_from_slice(&[0x0F, 0x9C, 0xC0]); // setl al
        self.movzx_rax_al();
    }
    fn test_acc(&mut self) { self.test_rax_rax(); }

    fn je_rel32(&mut self) -> usize { self.je_rel32() }
    fn jne_rel32(&mut self) -> usize { self.jne_rel32() }
    fn jmp_rel32(&mut self) -> usize { self.jmp_rel32() }
    fn nop(&mut self) { self.nop(); }
    fn syscall_inst(&mut self) { self.syscall(); }
    fn ret(&mut self) { self.ret(); }

    fn call_rel32(&mut self) -> usize { self.call_rel32() }

    fn patch_string_ref(&mut self, d: usize, r: usize, f: usize) { self.patch_string_ref(d, r, f); }
    fn patch_rel32(&mut self, offset: usize, from: usize, to: usize) { self.patch_rel32(offset, from, to); }

    fn emit_prologue(&mut self) -> usize {
        self.push_rbp();
        self.mov_rbp_rsp();
        let off = self.here();
        self.sub_rsp_imm32(0); // placeholder
        off
    }

    fn patch_frame_size(&mut self, prologue_offset: usize, frame_size: u32) {
        let aligned = (frame_size + 15) & !15;
        let imm_bytes = (aligned as i32).to_le_bytes();
        self.bytes[prologue_offset + 3] = imm_bytes[0];
        self.bytes[prologue_offset + 4] = imm_bytes[1];
        self.bytes[prologue_offset + 5] = imm_bytes[2];
        self.bytes[prologue_offset + 6] = imm_bytes[3];
    }

    fn emit_epilogue(&mut self) { self.leave(); self.ret(); }

    fn arg_reg_count(&self) -> usize { 7 }
    fn arg_reg(&self, i: usize) -> Option<u32> {
        const REGS: [u32; 7] = [7, 6, 2, 10, 8, 9, 0]; // RDI,RSI,RDX,R10,R8,R9,RAX
        REGS.get(i).copied()
    }
    fn ret_reg(&self) -> u32 { 0 } // RAX
    fn acc_reg(&self) -> u32 { 0 } // RAX
    fn scratch_reg(&self) -> u32 { 1 } // RCX

    fn parse_reg(&self, name: &str) -> Option<u32> {
        match name {
            "rax" => Some(0), "rcx" => Some(1), "rdx" => Some(2), "rbx" => Some(3),
            "rsp" => Some(4), "rbp" => Some(5), "rsi" => Some(6), "rdi" => Some(7),
            "r8"  => Some(8), "r9"  => Some(9), "r10" => Some(10), "r11" => Some(11),
            "r12" => Some(12), "r13" => Some(13), "r14" => Some(14), "r15" => Some(15),
            _ => None,
        }
    }

    fn intrinsic_bytes(&self, name: &str) -> Option<&'static [u8]> {
        match name {
            "syscall" => Some(&[0x0F, 0x05]),
            "nop"     => Some(&[0x90]),
            "pausa"   => Some(&[0xF3, 0x90]),
            "int3"    => Some(&[0xCC]),
            "hlt"     => Some(&[0xF4]),
            "cli"     => Some(&[0xFA]),
            "sti"     => Some(&[0xFB]),
            "rdtsc"   => Some(&[0x0F, 0x31]),
            "cpuid"   => Some(&[0x0F, 0xA2]),
            "lfence"  => Some(&[0x0F, 0xAE, 0xE8]),
            "mfence"  => Some(&[0x0F, 0xAE, 0xF0]),
            "sfence"  => Some(&[0x0F, 0xAE, 0xF8]),
            _ => None,
        }
    }
}
