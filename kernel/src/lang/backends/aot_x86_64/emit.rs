//! `backends::aot_x86_64::emit` — Emitter de bytes x86-64.
//!
//! Emite instrucciones x86-64 reales a un buffer de bytes. La
//! codificación sigue el manual Intel SDM Vol 2.
//!
//! ## Codificación típica
//!
//! ```text
//! [Prefixes 1B] [REX 1B] [Opcode 1-3B] [ModR/M 1B] [SIB 1B] [Displacement 0/1/2/4B] [Immediate 0/1/2/4/8B]
//! ```
//!
//! Este emitter es **simple**: soporta las instrucciones más comunes
//! (mov, add, sub, cmp, push, pop, call, ret, jmp, jcc, syscall, etc.)
//! lo suficiente para compilar el common IR.

#![allow(dead_code)]

use super::abi::Reg;

const CODE_BUF_SIZE: usize = 64 * 1024;

/// Emite bytes x86-64 a un buffer interno.
pub struct Emitter {
    code: [u8; CODE_BUF_SIZE],
    pub code_len: usize,
}

impl Emitter {
    pub const fn new() -> Self {
        Self { code: [0; CODE_BUF_SIZE], code_len: 0 }
    }

    pub fn bytes(&self) -> &[u8] { &self.code[..self.code_len] }

    fn emit_byte(&mut self, b: u8) {
        if self.code_len < CODE_BUF_SIZE {
            self.code[self.code_len] = b;
            self.code_len += 1;
        }
    }

    fn emit_bytes(&mut self, bs: &[u8]) {
        for &b in bs {
            self.emit_byte(b);
        }
    }

    /// Emite REX prefix: `0100 WRXB`.
    /// `w` = 64-bit operand, `r` = ext reg, `x` = ext index, `b` = ext rm.
    fn emit_rex(&mut self, w: bool, r: Reg, x: Reg, b: Reg) {
        let mut rex = 0x40u8;
        if w { rex |= 0x08; }
        if r.needs_rex_r() { rex |= 0x04; }
        if x.needs_rex_b() { rex |= 0x02; }
        if b.needs_rex_b() { rex |= 0x01; }
        self.emit_byte(rex);
    }

    /// `mov r64, imm64` — opcode 0xB8+rd.
    pub fn mov_rax_imm64(&mut self, imm: u64) {
        self.emit_rex(true, Reg::Rax, Reg::Rax, Reg::Rax);
        self.emit_byte(0xB8); // + rax.code3() = 0xB8
        self.emit_bytes(&imm.to_le_bytes());
    }

    /// `mov reg, imm32` (sign-extended to 64).
    pub fn mov_imm32(&mut self, dst: Reg, imm: i32) {
        self.emit_rex(true, Reg::Rax, Reg::Rax, dst);
        self.emit_byte(0xC7);
        self.emit_byte(0xC0 | dst.code3()); // ModR/M: mod=11, reg=0, rm=dst
        self.emit_bytes(&imm.to_le_bytes());
    }

    /// `mov reg, reg`.
    pub fn mov_rr(&mut self, dst: Reg, src: Reg) {
        self.emit_rex(true, src, Reg::Rax, dst);
        self.emit_byte(0x89);
        self.emit_byte(0xC0 | (src.code3() << 3) | dst.code3());
    }

    /// `add dst, imm32`.
    pub fn add_imm(&mut self, dst: Reg, imm: i32) {
        if imm >= -128 && imm <= 127 {
            self.emit_rex(true, Reg::Rax, Reg::Rax, dst);
            self.emit_byte(0x83);
            self.emit_byte(0xC0 | dst.code3());
            self.emit_byte(imm as u8);
        } else {
            self.emit_rex(true, Reg::Rax, Reg::Rax, dst);
            self.emit_byte(0x81);
            self.emit_byte(0xC0 | dst.code3());
            self.emit_bytes(&imm.to_le_bytes());
        }
    }

    /// `sub dst, imm32`.
    pub fn sub_imm(&mut self, dst: Reg, imm: i32) {
        if imm >= -128 && imm <= 127 {
            self.emit_rex(true, Reg::Rax, Reg::Rax, dst);
            self.emit_byte(0x83);
            self.emit_byte(0xE8 | dst.code3());
            self.emit_byte(imm as u8);
        } else {
            self.emit_rex(true, Reg::Rax, Reg::Rax, dst);
            self.emit_byte(0x81);
            self.emit_byte(0xE8 | dst.code3());
            self.emit_bytes(&imm.to_le_bytes());
        }
    }

    /// `push reg`.
    pub fn push(&mut self, r: Reg) {
        if r.needs_rex_b() {
            self.emit_byte(0x41);
        }
        self.emit_byte(0x50 | r.code3());
    }

    /// `pop reg`.
    pub fn pop(&mut self, r: Reg) {
        if r.needs_rex_b() {
            self.emit_byte(0x41);
        }
        self.emit_byte(0x58 | r.code3());
    }

    /// `call rel32` (relative to next instruction).
    pub fn call_rel32(&mut self, rel: i32) {
        self.emit_byte(0xE8);
        self.emit_bytes(&rel.to_le_bytes());
    }

    /// `ret`.
    pub fn ret(&mut self) {
        self.emit_byte(0xC3);
    }

    /// `syscall` (ring 0 transition via BMO ABI).
    pub fn syscall(&mut self) {
        self.emit_bytes(&[0x0F, 0x05]);
    }

    /// `jmp rel32`.
    pub fn jmp_rel32(&mut self, rel: i32) {
        self.emit_byte(0xE9);
        self.emit_bytes(&rel.to_le_bytes());
    }

    /// `jcc rel32` (conditional jump).
    pub fn jcc(&mut self, cc: CondCode, rel: i32) {
        self.emit_byte(0x0F);
        self.emit_byte(0x80 | cc as u8);
        self.emit_bytes(&rel.to_le_bytes());
    }

    /// `xor reg, reg` (zero reg).
    pub fn xor_rr(&mut self, dst: Reg, src: Reg) {
        self.emit_rex(true, src, Reg::Rax, dst);
        self.emit_byte(0x31);
        self.emit_byte(0xC0 | (src.code3() << 3) | dst.code3());
    }

    /// `mov rbp, rsp` (frame setup).
    pub fn mov_rbp_rsp(&mut self) {
        self.mov_rr(Reg::Rbp, Reg::Rsp);
    }

    /// `sub rsp, imm32` (alloc stack frame).
    pub fn sub_rsp_imm(&mut self, imm: i32) {
        self.sub_imm(Reg::Rsp, imm);
    }

    /// `add rsp, imm32` (dealloc stack frame).
    pub fn add_rsp_imm(&mut self, imm: i32) {
        self.add_imm(Reg::Rsp, imm);
    }

    /// Patchea un `rel32` que ya fue emitido a `target` desde `from`.
    pub fn patch_rel32(&mut self, at: usize, target: usize) {
        let cur = at + 4;
        let rel = (target as isize - cur as isize) as i32;
        let bytes = rel.to_le_bytes();
        self.code[at..at+4].copy_from_slice(&bytes);
    }

    /// Posición actual de emisión.
    pub fn pos(&self) -> usize { self.code_len }

    /// Reserva 4 bytes para un `rel32` y devuelve la posición.
    pub fn reserve_rel32(&mut self) -> usize {
        let p = self.code_len;
        self.emit_bytes(&[0; 4]);
        p
    }
}

/// Condition codes para `jcc`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CondCode {
    /// je / jz
    E   = 0x4,
    /// jne / jnz
    Ne  = 0x5,
    /// jl / jnge (signed)
    L   = 0xC,
    /// jle / jng
    Le  = 0xE,
    /// jg / jnle
    G   = 0xF,
    /// jge / jnl
    Ge  = 0xD,
    /// jb / jnae / jc
    B   = 0x2,
    /// jbe / jna
    Be  = 0x6,
    /// ja / jnbe
    A   = 0x7,
    /// jae / jnb / jnc
    Ae  = 0x3,
    /// js (sign)
    S   = 0x8,
    /// jns
    Ns  = 0x9,
}
