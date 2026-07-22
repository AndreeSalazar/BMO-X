//! Ensamblador x86-64 mínimo sobre el motor sem-asm.
//!
//! Encodea el subconjunto que HOY los `codegen.rs` de C y COBOL escriben a
//! mano (mov reg,imm64 · mov reg,reg · syscall). El **opcode** sale de la
//! tabla TOML (`Instructions`); el REX/ModRM lo compone esta capa según las
//! reglas de x86-64. Migrar los codegens = reemplazar los bytes hardcodeados
//! por llamadas a `Asm`.

use crate::{Instructions, SemAsmError};

/// Registros de 64 bits con su numeración de encoding x86-64.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Reg {
    Rax = 0, Rcx = 1, Rdx = 2, Rbx = 3,
    Rsp = 4, Rbp = 5, Rsi = 6, Rdi = 7,
    R8 = 8, R9 = 9, R10 = 10, R11 = 11,
    R12 = 12, R13 = 13, R14 = 14, R15 = 15,
}

impl Reg {
    #[inline]
    fn num(self) -> u8 { self as u8 }
    #[inline]
    fn low3(self) -> u8 { self.num() & 0x7 }
    #[inline]
    fn ext(self) -> bool { self.num() >= 8 }
}

/// Prefijo REX.W (operando de 64 bits) con los bits R/B según extensión.
fn rex_w(r_ext: bool, b_ext: bool) -> u8 {
    0x48 | (if r_ext { 0x4 } else { 0 }) | (if b_ext { 0x1 } else { 0 })
}

/// Ensamblador que acumula bytes, tomando opcodes de la tabla TOML.
pub struct Asm<'a> {
    isa: &'a Instructions,
    out: Vec<u8>,
}

impl<'a> Asm<'a> {
    pub fn new(isa: &'a Instructions) -> Self {
        Self { isa, out: Vec::new() }
    }

    pub fn into_bytes(self) -> Vec<u8> { self.out }
    pub fn bytes(&self) -> &[u8] { &self.out }

    /// `mov r64, imm64` — opcode base `mov_imm` (0xB8) + reg, con REX.W e imm64 LE.
    pub fn mov_imm64(&mut self, dst: Reg, imm: u64) -> Result<&mut Self, SemAsmError> {
        let base = self.isa.opcode("mov_imm")?[0];
        self.out.push(rex_w(false, dst.ext()));
        self.out.push(base + dst.low3());
        self.out.extend_from_slice(&imm.to_le_bytes());
        Ok(self)
    }

    /// `mov r/m64, r64` — opcode `mov` (0x89), ModRM en modo registro (11).
    /// Semántica: `dst = src`. dst es el r/m, src es el reg del ModRM.
    pub fn mov_reg(&mut self, dst: Reg, src: Reg) -> Result<&mut Self, SemAsmError> {
        let op = self.isa.opcode("mov")?[0]; // 0x89 (mov r/m, r)
        self.out.push(rex_w(src.ext(), dst.ext()));
        self.out.push(op);
        // ModRM: mod=11, reg=src, r/m=dst.
        self.out.push(0xC0 | (src.low3() << 3) | dst.low3());
        Ok(self)
    }

    /// `syscall` (0F 05). No está en `instructions.toml`; es un opcode fijo
    /// de 2 bytes, así que se emite directo.
    pub fn syscall(&mut self) -> &mut Self {
        self.out.extend_from_slice(&[0x0F, 0x05]);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isa() -> Instructions { Instructions::load_x86_64().unwrap() }

    #[test]
    fn encodes_mov_imm64_rax() {
        let isa = isa();
        let mut a = Asm::new(&isa);
        a.mov_imm64(Reg::Rax, 0xFFFF_FFFF_FFFF_FFFE).unwrap();
        // 48 B8 FE FF FF FF FF FF FF FF  (mov rax, CURRENT_TASK)
        assert_eq!(a.bytes()[0], 0x48);
        assert_eq!(a.bytes()[1], 0xB8);
        assert_eq!(&a.bytes()[2..], &0xFFFF_FFFF_FFFF_FFFEu64.to_le_bytes());
    }

    #[test]
    fn encodes_mov_reg_reg() {
        let isa = isa();
        let mut a = Asm::new(&isa);
        a.mov_reg(Reg::Rdi, Reg::Rax).unwrap(); // mov rdi, rax
        assert_eq!(a.bytes(), &[0x48, 0x89, 0xC7]);
        let mut b = Asm::new(&isa);
        b.mov_reg(Reg::Rsi, Reg::Rax).unwrap(); // mov rsi, rax
        assert_eq!(b.bytes(), &[0x48, 0x89, 0xC6]);
    }

    #[test]
    fn encodes_the_full_invoke_prologue() {
        // Exactamente la secuencia que el codegen de COBOL hoy hardcodea:
        // mov rax, CURRENT_TASK ; mov rdi, rax ; ... ; syscall
        let isa = isa();
        let mut a = Asm::new(&isa);
        a.mov_imm64(Reg::Rax, 0xFFFF_FFFF_FFFF_FFFE).unwrap();
        a.mov_reg(Reg::Rdi, Reg::Rax).unwrap();
        a.syscall();
        let b = a.into_bytes();
        assert_eq!(&b[..2], &[0x48, 0xB8]);
        assert_eq!(&b[10..13], &[0x48, 0x89, 0xC7]);
        assert_eq!(&b[13..], &[0x0F, 0x05]);
    }
}
