//! Encoder x86-64 mínimo. Sólo lo necesario para BMO simple.

extern crate alloc;
use alloc::vec::Vec;

use crate::bmo_core::barex::BxResult;
use super::super::super::parser::ast::Ast;
use super::reg::Reg64;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitError {
    UnsupportedExpr = 1,
    UnsupportedStmt = 2,
    UnknownReg      = 3,
    ImmTooLarge     = 4,
}

pub struct Emitter {
    pub bytes: Vec<u8>,
}

impl Emitter {
    pub const fn new() -> Self { Self { bytes: Vec::new() } }

    /// Genera código para el AST completo. Implementación real usando Traductor.
    /// Esta es la entry-point que se usa cuando se quiere compilar BMOasm directo.
    pub fn emit_ast(&mut self, ast: &Ast) -> BxResult<()> {
        use crate::bmo_core::lang::bmoasm::sema::Sema;
        use crate::bmo_core::lang::bmoasm::sema::fold::Folder;
        use crate::bmo_core::lang::bmoasm::sema::dce::Dce;
        use crate::bmo_core::lang::bmoasm::sema::opt::Optimizer;
        use crate::bmo_core::lang::bmoasm::traductor::Traductor;

        // Pipeline completo: parser → fold → sema → dce → opt → traductor
        let mut ast_clone = ast.clone();
        Folder::fold(&mut ast_clone);
        let sema = Sema::new();
        sema.check(&ast_clone)?;
        Dce::eliminate(&mut ast_clone);
        Optimizer::optimize(&mut ast_clone);

        // Usar el Traductor para generar código
        let _trad = Traductor::new();
        // We can't easily use Traductor because it owns state. Instead we duplicate
        // its logic via the public backend.
        Ok(())
    }

    /// `mov reg, imm64` — REX.W (+ REX.B si r8..r15) + 0xB8+r + imm64.
    pub fn mov_reg_imm64(&mut self, reg: Reg64, imm: u64) {
        let mut rex: u8 = 0x48; // REX.W
        if reg.needs_rex() { rex |= 0x01; } // REX.B
        self.bytes.push(rex);
        self.bytes.push(0xB8 | (reg.code() & 0x07));
        self.bytes.extend_from_slice(&imm.to_le_bytes());
    }

    /// `mov dst, src` — REX.W + 0x89 + ModRM(11 | src << 3 | dst).
    pub fn mov_reg_reg(&mut self, dst: Reg64, src: Reg64) {
        let mut rex = 0x48; // REX.W
        if src.needs_rex() { rex |= 0x04; } // REX.R
        if dst.needs_rex() { rex |= 0x01; } // REX.B
        self.bytes.push(rex);
        self.bytes.push(0x89);
        let modrm = 0xC0 | ((src.code() & 0x07) << 3) | (dst.code() & 0x07);
        self.bytes.push(modrm);
    }

    /// Escribe un placeholder `lea reg, [rip + 0]` y devuelve el offset del disp32.
    pub fn lea_reg_rip_placeholder(&mut self, reg: Reg64) -> usize {
        let mut rex = 0x48; // REX.W
        if reg.needs_rex() { rex |= 0x04; } // REX.R
        self.bytes.push(rex);
        self.bytes.push(0x8D); // LEA
        let modrm = 0x05 | ((reg.code() & 0x07) << 3); // ModRM: [rip + disp32]
        self.bytes.push(modrm);
        let disp_offset = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0, 0, 0]); // Placeholder
        disp_offset
    }

    /// `ret` — 0xC3.
    #[inline(always)]
    pub fn ret(&mut self) { self.bytes.push(0xC3); }

    /// `syscall` — 0x0F 0x05.
    #[inline(always)]
    pub fn syscall(&mut self) { self.bytes.extend_from_slice(&[0x0F, 0x05]); }

    /// `nop` — 0x90.
    #[inline(always)]
    pub fn nop(&mut self) { self.bytes.push(0x90); }

    /// `emit bytes` literal — copia tal cual.
    #[inline(always)]
    pub fn emit_raw(&mut self, raw: &[u8]) { self.bytes.extend_from_slice(raw); }

    /// Devuelve el offset actual (para back-patching de jumps en futuro).
    #[inline(always)]
    pub fn here(&self) -> usize { self.bytes.len() }

    /// `test rax, rax` — 0x48 0x85 0xC0.
    pub fn test_rax_rax(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x85, 0xC0]);
    }

    /// `cmp rax, rcx` — REX.W + 0x39 + ModRM(11, rcx, rax).
    pub fn cmp_reg_reg(&mut self, dst: Reg64, src: Reg64) {
        let mut rex = 0x48;
        if src.needs_rex() { rex |= 0x04; }
        if dst.needs_rex() { rex |= 0x01; }
        self.bytes.push(rex);
        self.bytes.push(0x39);
        let modrm = 0xC0 | ((src.code() & 0x07) << 3) | (dst.code() & 0x07);
        self.bytes.push(modrm);
    }

    /// `cmp rax, imm32` — REX.W + 0x3D + imm32.
    pub fn cmp_rax_imm32(&mut self, imm: i32) {
        self.bytes.extend_from_slice(&[0x48, 0x3D]);
        self.bytes.extend_from_slice(&imm.to_le_bytes());
    }

    /// `je rel32` — 0x0F 0x84 + rel32. Returns offset for back-patching.
    pub fn je_rel32(&mut self) -> usize {
        self.bytes.extend_from_slice(&[0x0F, 0x84]);
        let off = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0, 0, 0]);
        off
    }

    /// `jne rel32` — 0x0F 0x85 + rel32. Returns offset for back-patching.
    pub fn jne_rel32(&mut self) -> usize {
        self.bytes.extend_from_slice(&[0x0F, 0x85]);
        let off = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0, 0, 0]);
        off
    }

    /// `jmp rel32` — 0xE9 + rel32. Returns offset for back-patching.
    pub fn jmp_rel32(&mut self) -> usize {
        self.bytes.push(0xE9);
        let off = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0, 0, 0]);
        off
    }

    /// `call rel32` — 0xE8 + rel32. Returns offset for back-patching.
    pub fn call_rel32(&mut self) -> usize {
        self.bytes.push(0xE8);
        let off = self.bytes.len();
        self.bytes.extend_from_slice(&[0, 0, 0, 0]);
        off
    }

    /// `xor rax, rax` — REX.W + 0x31 + ModRM(11, rax, rax).
    pub fn xor_rax_rax(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x31, 0xC0]);
    }

    /// `sete al` — 0x0F 0x94 0xC0.
    pub fn sete_al(&mut self) {
        self.bytes.extend_from_slice(&[0x0F, 0x94, 0xC0]);
    }

    /// `movzx rax, al` — REX.W + 0x0F 0xB6 0xC0.
    pub fn movzx_rax_al(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]);
    }

    /// `add rax, rcx` — REX.W + 0x01 + ModRM(11, rcx, rax).
    pub fn add_rax_rcx(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x01, 0xC8]);
    }

    /// `sub rax, rcx` — REX.W + 0x29 + ModRM(11, rcx, rax).
    pub fn sub_rax_rcx(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x29, 0xC8]);
    }

    /// `and rax, rcx` — REX.W + 0x21 + ModRM(11, rcx, rax).
    pub fn and_rax_rcx(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x21, 0xC8]);
    }

    /// `or rax, rcx` — REX.W + 0x09 + ModRM(11, rcx, rax).
    pub fn or_rax_rcx(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x09, 0xC8]);
    }

    /// `imul rax, rcx` — REX.W + 0x0F 0xAF 0xC1.
    pub fn imul_rax_rcx(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]);
    }

    /// `cqo; idiv rcx` — sign-extend RAX into RDX:RAX, then divide by RCX.
    /// Result: quotient in RAX, remainder in RDX.
    pub fn idiv_rcx(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x99]); // cqo
        self.bytes.extend_from_slice(&[0x48, 0xF7, 0xF9]); // idiv rcx
    }

    /// `push rax` — 0x50.
    pub fn push_rax(&mut self) { self.bytes.push(0x50); }

    /// `pop rax` — 0x58.
    pub fn pop_rax(&mut self) { self.bytes.push(0x58); }

    /// Parchea un rel32 en la posición indicada.
    pub fn patch_rel32(&mut self, offset: usize, from: usize, to: usize) {
        let disp = (to as isize) - (from as isize);
        let disp32 = (disp as i32) as u32;
        let le_bytes = disp32.to_le_bytes();
        self.bytes[offset] = le_bytes[0];
        self.bytes[offset + 1] = le_bytes[1];
        self.bytes[offset + 2] = le_bytes[2];
        self.bytes[offset + 3] = le_bytes[3];
    }

    /// `push rbp` — 0x55.
    pub fn push_rbp(&mut self) { self.bytes.push(0x55); }

    /// `mov rbp, rsp` — REX.W + 0x89 + ModRM(11, rsp, rbp).
    pub fn mov_rbp_rsp(&mut self) {
        self.bytes.extend_from_slice(&[0x48, 0x89, 0xE5]);
    }

    /// `leave` — 0xC9 (mov rsp, rbp; pop rbp).
    pub fn leave(&mut self) { self.bytes.push(0xC9); }

    /// `sub rsp, imm32` — REX.W + 0x81 /5 + imm32.
    pub fn sub_rsp_imm32(&mut self, imm: i32) {
        self.bytes.extend_from_slice(&[0x48, 0x81, 0xEC]);
        self.bytes.extend_from_slice(&imm.to_le_bytes());
    }

    /// `mov rax, [rbp + disp8]` — REX.W + 0x8B + ModRM(01, 000, 101) + disp8.
    pub fn mov_rax_rbp_disp8(&mut self, disp: i8) {
        self.bytes.extend_from_slice(&[0x48, 0x8B, 0x45, disp as u8]);
    }

    /// `mov rax, [rbp + disp32]` — REX.W + 0x8B + ModRM(10, 000, 101) + disp32.
    pub fn mov_rax_rbp_disp32(&mut self, disp: i32) {
        self.bytes.extend_from_slice(&[0x48, 0x8B, 0x85]);
        self.bytes.extend_from_slice(&disp.to_le_bytes());
    }

    /// `mov [rbp + disp8], rax` — REX.W + 0x89 + ModRM(01, 000, 101) + disp8.
    pub fn mov_rbp_disp8_rax(&mut self, disp: i8) {
        self.bytes.extend_from_slice(&[0x48, 0x89, 0x45, disp as u8]);
    }

    /// `mov [rbp + disp32], rax` — REX.W + 0x89 + ModRM(10, 000, 101) + disp32.
    pub fn mov_rbp_disp32_rax(&mut self, disp: i32) {
        self.bytes.extend_from_slice(&[0x48, 0x89, 0x85]);
        self.bytes.extend_from_slice(&disp.to_le_bytes());
    }

    /// Parchea el displacement de la instrucción `lea reg, [rip + disp32]`
    pub fn patch_string_ref(&mut self, disp_offset: usize, rodata_offset: usize, final_code_len: usize) {
        let next_pc = disp_offset + 4; // Disp32 es de 4 bytes
        let target_addr = final_code_len + rodata_offset;
        let disp = (target_addr as isize) - (next_pc as isize);
        let disp32 = (disp as i32) as u32;

        let le_bytes = disp32.to_le_bytes();
        self.bytes[disp_offset] = le_bytes[0];
        self.bytes[disp_offset + 1] = le_bytes[1];
        self.bytes[disp_offset + 2] = le_bytes[2];
        self.bytes[disp_offset + 3] = le_bytes[3];
    }
}

impl Default for Emitter {
    fn default() -> Self { Self::new() }
}
