//! v1.0 — BMO Bytecode Virtual Machine.
//!
//! Stack-based executor for the 32-opcode BMO bytecode format.
//! Pure no_std, fixed-size arrays (no heap allocation).

#![allow(dead_code)]

use super::emit;
use crate::bmo_core::diag;

const STACK_MAX: usize = 1024;
const LOCALS_MAX: usize = 256;
const CALL_DEPTH_MAX: usize = 64;

pub struct BmoVm {
    stack: [u64; STACK_MAX],
    sp: usize,
    locals: [u64; LOCALS_MAX],
    call_sp: usize,
    call_stack: [CallFrame; CALL_DEPTH_MAX],
}

#[derive(Clone, Copy)]
struct CallFrame {
    ret_pc: usize,
    base_local: u8,
}

pub enum VmExit {
    Halted,
    Error(&'static str),
}

macro_rules! binop {
    ($vm:expr, $op:tt) => {
        if $vm.sp < 2 { return VmExit::Error("stack underflow"); }
        let b = $vm.stack[$vm.sp - 1];
        let a = $vm.stack[$vm.sp - 2];
        $vm.stack[$vm.sp - 2] = a $op b;
        $vm.sp -= 1;
    };
}

macro_rules! binop_shift {
    ($vm:expr, $op:tt) => {
        if $vm.sp < 2 { return VmExit::Error("stack underflow"); }
        let b = $vm.stack[$vm.sp - 1];
        let a = $vm.stack[$vm.sp - 2];
        $vm.stack[$vm.sp - 2] = a $op (b as u32);
        $vm.sp -= 1;
    };
}

macro_rules! cmpop {
    ($vm:expr, $op:tt) => {
        if $vm.sp < 2 { return VmExit::Error("stack underflow"); }
        let b = $vm.stack[$vm.sp - 1];
        let a = $vm.stack[$vm.sp - 2];
        $vm.stack[$vm.sp - 2] = if a $op b { 1 } else { 0 };
        $vm.sp -= 1;
    };
}

macro_rules! logbinop {
    ($vm:expr, $op:tt) => {
        if $vm.sp < 2 { return VmExit::Error("stack underflow"); }
        let b = $vm.stack[$vm.sp - 1] != 0;
        let a = $vm.stack[$vm.sp - 2] != 0;
        $vm.stack[$vm.sp - 2] = if a $op b { 1 } else { 0 };
        $vm.sp -= 1;
    };
}

impl BmoVm {
    pub fn new() -> Self {
        Self {
            stack: [0u64; STACK_MAX],
            sp: 0,
            locals: [0u64; LOCALS_MAX],
            call_sp: 0,
            call_stack: [CallFrame { ret_pc: 0, base_local: 0 }; CALL_DEPTH_MAX],
        }
    }

    pub fn execute(&mut self, code: &[u8]) -> VmExit {
        let mut pc: usize = 0;
        loop {
            if pc >= code.len() {
                return VmExit::Error("PC out of bounds");
            }
            let op = code[pc];
            pc += 1;
            match op {
                emit::NOP => {}
                emit::PUSH_IMM64 => {
                    if pc + 8 > code.len() { return VmExit::Error("truncated PUSH_IMM64"); }
                    let val = u64::from_le_bytes([
                        code[pc], code[pc+1], code[pc+2], code[pc+3],
                        code[pc+4], code[pc+5], code[pc+6], code[pc+7],
                    ]);
                    pc += 8;
                    if self.sp >= STACK_MAX { return VmExit::Error("stack overflow"); }
                    self.stack[self.sp] = val;
                    self.sp += 1;
                }
                emit::POP => {
                    if self.sp == 0 { return VmExit::Error("stack underflow (POP)"); }
                    self.sp -= 1;
                }
                emit::ADD => { binop!(self, +); }
                emit::SUB => { binop!(self, -); }
                emit::MUL => { binop!(self, *); }
                emit::DIV => {
                    if self.sp < 2 { return VmExit::Error("stack underflow (DIV)"); }
                    let b = self.stack[self.sp - 1];
                    let a = self.stack[self.sp - 2];
                    if b == 0 { return VmExit::Error("division by zero"); }
                    self.stack[self.sp - 2] = a / b;
                    self.sp -= 1;
                }
                emit::MOD => {
                    if self.sp < 2 { return VmExit::Error("stack underflow (MOD)"); }
                    let b = self.stack[self.sp - 1];
                    let a = self.stack[self.sp - 2];
                    if b == 0 { return VmExit::Error("division by zero"); }
                    self.stack[self.sp - 2] = a % b;
                    self.sp -= 1;
                }
                emit::AND => { binop!(self, &); }
                emit::OR => { binop!(self, |); }
                emit::XOR => { binop!(self, ^); }
                emit::SHL => { binop_shift!(self, <<); }
                emit::SHR => { binop_shift!(self, >>); }
                emit::EQ => { cmpop!(self, ==); }
                emit::NE => { cmpop!(self, !=); }
                emit::LT => { cmpop!(self, <); }
                emit::GT => { cmpop!(self, >); }
                emit::LE => { cmpop!(self, <=); }
                emit::GE => { cmpop!(self, >=); }
                emit::LAND => { logbinop!(self, &&); }
                emit::LOR => { logbinop!(self, ||); }
                emit::NOT => {
                    if self.sp == 0 { return VmExit::Error("stack underflow (NOT)"); }
                    self.stack[self.sp - 1] = if self.stack[self.sp - 1] == 0 { 1 } else { 0 };
                }
                emit::NEG => {
                    if self.sp == 0 { return VmExit::Error("stack underflow (NEG)"); }
                    self.stack[self.sp - 1] = (-(self.stack[self.sp - 1] as i64)) as u64;
                }
                emit::LOAD_LOCAL => {
                    if pc >= code.len() { return VmExit::Error("truncated LOAD_LOCAL"); }
                    let slot = code[pc] as usize;
                    pc += 1;
                    if slot >= LOCALS_MAX { return VmExit::Error("local slot out of range"); }
                    if self.sp >= STACK_MAX { return VmExit::Error("stack overflow"); }
                    self.stack[self.sp] = self.locals[slot];
                    self.sp += 1;
                }
                emit::STORE_LOCAL => {
                    if pc >= code.len() { return VmExit::Error("truncated STORE_LOCAL"); }
                    let slot = code[pc] as usize;
                    pc += 1;
                    if slot >= LOCALS_MAX { return VmExit::Error("local slot out of range"); }
                    if self.sp == 0 { return VmExit::Error("stack underflow (STORE_LOCAL)"); }
                    self.sp -= 1;
                    self.locals[slot] = self.stack[self.sp];
                }
                emit::CALL => {
                    if pc >= code.len() { return VmExit::Error("truncated CALL"); }
                    let slot = code[pc] as usize;
                    pc += 1;
                    let target = self.locals[slot];
                    if target == 0 {
                        return VmExit::Error("CALL to null function pointer");
                    }
                    if self.call_sp >= CALL_DEPTH_MAX {
                        return VmExit::Error("call stack overflow");
                    }
                    self.call_stack[self.call_sp] = CallFrame {
                        ret_pc: pc,
                        base_local: 0,
                    };
                    self.call_sp += 1;
                    pc = target as usize;
                }
                emit::RET => {
                    if self.call_sp == 0 {
                        return VmExit::Halted;
                    }
                    self.call_sp -= 1;
                    pc = self.call_stack[self.call_sp].ret_pc;
                }
                emit::JMP => {
                    if pc + 2 > code.len() { return VmExit::Error("truncated JMP"); }
                    let offset = i16::from_le_bytes([code[pc], code[pc+1]]);
                    pc = (pc as isize + offset as isize) as usize;
                }
                emit::JMP_IF_FALSE => {
                    if pc + 2 > code.len() { return VmExit::Error("truncated JMP_IF_FALSE"); }
                    let offset = i16::from_le_bytes([code[pc], code[pc+1]]);
                    pc += 2;
                    if self.sp == 0 { return VmExit::Error("stack underflow (JMP_IF_FALSE)"); }
                    self.sp -= 1;
                    if self.stack[self.sp] == 0 {
                        pc = (pc as isize + offset as isize) as usize;
                    }
                }
                emit::SYS_CALL => {
                    if pc >= code.len() { return VmExit::Error("truncated SYS_CALL"); }
                    let nr = code[pc];
                    pc += 1;
                    let arg_count = self.sp;
                    let mut args = [0u64; 6];
                    let n = arg_count.min(6);
                    for i in 0..n {
                        self.sp -= 1;
                        args[n - 1 - i] = self.stack[self.sp];
                    }
                    let result = crate::bmo_core::bmo_api::dispatch_syscall(
                        nr as u16, args[0], args[1], args[2], args[3], args[4], args[5],
                    );
                    if self.sp >= STACK_MAX { return VmExit::Error("stack overflow after syscall"); }
                    self.stack[self.sp] = result;
                    self.sp += 1;
                }
                emit::DUP => {
                    if self.sp == 0 { return VmExit::Error("stack underflow (DUP)"); }
                    if self.sp >= STACK_MAX { return VmExit::Error("stack overflow (DUP)"); }
                    let top = self.stack[self.sp - 1];
                    self.stack[self.sp] = top;
                    self.sp += 1;
                }
                emit::SWAP => {
                    if self.sp < 2 { return VmExit::Error("stack underflow (SWAP)"); }
                    let a = self.stack[self.sp - 1];
                    let b = self.stack[self.sp - 2];
                    self.stack[self.sp - 1] = b;
                    self.stack[self.sp - 2] = a;
                }
                emit::HALT => {
                    return VmExit::Halted;
                }
                _ => {
                    diag::warn("bmo_vm", "Unknown opcode");
                    return VmExit::Error("unknown opcode");
                }
            }
        }
    }

    pub fn stack_top(&self) -> Option<u64> {
        if self.sp > 0 { Some(self.stack[self.sp - 1]) } else { None }
    }

    pub fn stack_depth(&self) -> usize { self.sp }

    pub fn reset(&mut self) {
        self.sp = 0;
        self.locals = [0u64; LOCALS_MAX];
        self.call_sp = 0;
    }
}

impl Default for BmoVm {
    fn default() -> Self { Self::new() }
}
