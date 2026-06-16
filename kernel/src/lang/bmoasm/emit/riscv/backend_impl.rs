//! Backend RISC-V — stub implementation de `CodegenBackend`.
//! Futuro: implementación completa para RV64GC.

use super::super::backend::CodegenBackend;
use super::EmitterRiscv;

impl CodegenBackend for EmitterRiscv {
    fn emit_bytes(&mut self, bytes: &[u8]) { self.emit_raw(bytes); }
    fn here(&self) -> usize { self.here() }
    fn bytes_mut(&mut self) -> &mut alloc::vec::Vec<u8> { &mut self.bytes }

    fn mov_acc_imm(&mut self, imm: u64) { self.mov_reg_imm64(super::RegRiscv::A0, imm); }
    fn mov_acc_reg(&mut self, _src: u32) { /* TODO */ }
    fn mov_reg_acc(&mut self, _dst: u32) { /* TODO */ }
    fn mov_reg_reg(&mut self, _dst: u32, _src: u32) { /* TODO */ }

    fn load_var(&mut self, _frame_offset: i32) { /* TODO */ }
    fn store_var(&mut self, _frame_offset: i32) { /* TODO */ }

    fn push_acc(&mut self) { /* TODO */ }
    fn pop_acc(&mut self) { /* TODO */ }
    fn add_acc(&mut self, _reg: u32) { /* TODO */ }
    fn sub_acc(&mut self, _reg: u32) { /* TODO */ }
    fn mul_acc(&mut self, _reg: u32) { /* TODO */ }
    fn div_acc(&mut self, _reg: u32) { /* TODO */ }
    fn mod_acc(&mut self, _reg: u32) { /* TODO */ }
    fn and_acc(&mut self, _reg: u32) { /* TODO */ }
    fn or_acc(&mut self, _reg: u32) { /* TODO */ }
    fn xor_acc(&mut self, _reg: u32) { /* TODO */ }
    fn shl_acc(&mut self, _reg: u32) { /* TODO */ }
    fn shr_acc(&mut self, _reg: u32) { /* TODO */ }
    fn zero_acc(&mut self) { self.mov_reg_imm64(super::RegRiscv::A0, 0); }
    fn cmp_eq_acc(&mut self, _reg: u32) { /* TODO */ }
    fn cmp_gt_acc(&mut self, _reg: u32) { /* TODO */ }
    fn cmp_lt_acc(&mut self, _reg: u32) { /* TODO */ }
    fn sete_acc(&mut self) { /* TODO */ }
    fn setg_acc(&mut self) { /* TODO */ }
    fn setl_acc(&mut self) { /* TODO */ }
    fn test_acc(&mut self) { /* TODO */ }

    fn je_rel32(&mut self) -> usize { 0 /* TODO */ }
    fn jne_rel32(&mut self) -> usize { 0 /* TODO */ }
    fn jmp_rel32(&mut self) -> usize { 0 /* TODO */ }
    fn nop(&mut self) { self.nop(); }
    fn syscall_inst(&mut self) { self.syscall(); }
    fn ret(&mut self) { self.ret(); }

    fn call_rel32(&mut self) -> usize { 0 /* TODO */ }

    fn patch_string_ref(&mut self, d: usize, r: usize, f: usize) { self.patch_string_ref(d, r, f); }
    fn patch_rel32(&mut self, _offset: usize, _from: usize, _to: usize) { /* TODO */ }

    fn emit_prologue(&mut self) -> usize { 0 /* TODO */ }
    fn patch_frame_size(&mut self, _prologue_offset: usize, _frame_size: u32) { /* TODO */ }
    fn emit_epilogue(&mut self) { self.ret(); }

    fn arg_reg_count(&self) -> usize { 8 }
    fn arg_reg(&self, i: usize) -> Option<u32> {
        if i < 8 { Some((10 + i) as u32) } else { None } // A0-A7
    }
    fn ret_reg(&self) -> u32 { 10 }  // A0
    fn acc_reg(&self) -> u32 { 10 }   // A0
    fn scratch_reg(&self) -> u32 { 11 } // A1

    fn parse_reg(&self, name: &str) -> Option<u32> {
        match name {
            "zero" => Some(0), "ra" => Some(1), "sp" => Some(2), "gp" => Some(3),
            "tp" => Some(4), "t0" => Some(5), "t1" => Some(6), "t2" => Some(7),
            "s0" => Some(8), "s1" => Some(9),
            "a0" => Some(10), "a1" => Some(11), "a2" => Some(12), "a3" => Some(13),
            "a4" => Some(14), "a5" => Some(15), "a6" => Some(16), "a7" => Some(17),
            _ => None,
        }
    }

    fn intrinsic_bytes(&self, name: &str) -> Option<&'static [u8]> {
        match name {
            "syscall" => Some(&[0x73, 0x00, 0x00, 0x00]), // ECALL
            "nop"     => Some(&[0x13, 0x00, 0x00, 0x00]), // ADDI x0, x0, 0
            _ => None,
        }
    }
}
