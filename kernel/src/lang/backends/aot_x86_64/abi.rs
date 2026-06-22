//! `backends::aot_x86_64::abi` — Convencion de llamada SysV AMD64.
//!
//! Documenta la convención que el backend **debe** respetar para que
//! las funciones generadas puedan ser llamadas desde C, BMO, o
//! el kernel.
//!
//! ## Registros (SysV AMD64)
//!
//! - **Argumentos** (en orden): `RDI, RSI, RDX, RCX, R8, R9`
//! - **Retorno**: `RAX` (y `RDX` para structs grandes)
//! - **Callee-saved** (preservados): `RBX, RBP, R12..R15`
//! - **Caller-saved** (pueden ser pisados): `RAX, RCX, RDX, RSI, RDI, R8..R11`
//! - **Stack pointer**: `RSP` debe estar alineado a 16 bytes antes de `call`.
//!
//! ## Red zone
//!
//! 128 bytes bajo `RSP` que la función puede usar sin ajustar el stack.
//!
//! ## BMO ABI integration
//!
//! Cuando el código generado llama a una función BMO ABI (syscall),
//! se usa el registro `RAX` con el número de syscall. Esto es
//! compatible con SysV AMD64 (el caller pone argumentos en `RDI..R9`
//! y el callee hace `syscall` con `RAX`).

#![allow(dead_code)]

/// Argumentos de SysV AMD64 en orden.
pub const ARG_REGS: [Reg; 6] = [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8, Reg::R9];

/// Registros de retorno.
pub const RET_REGS: [Reg; 2] = [Reg::Rax, Reg::Rdx];

/// Registros callee-saved.
pub const CALLEE_SAVED: [Reg; 6] = [Reg::Rbx, Reg::Rbp, Reg::R12, Reg::R13, Reg::R14, Reg::R15];

/// Red zone size (128 bytes).
pub const RED_ZONE: u32 = 128;

/// Stack alignment al hacer `call` (16 bytes).
pub const STACK_ALIGN: u32 = 16;

/// Registro x86-64.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Reg {
    Rax = 0, Rcx, Rdx, Rbx, Rsp, Rbp, Rsi, Rdi,
    R8, R9, R10, R11, R12, R13, R14, R15,
}

impl Reg {
    /// Código de 3 bits para instrucciones ModR/M.
    #[inline]
    pub const fn code3(self) -> u8 {
        match self {
            Self::Rax | Self::R8  => 0,
            Self::Rcx | Self::R9  => 1,
            Self::Rdx | Self::R10 => 2,
            Self::Rbx | Self::R11 => 3,
            Self::Rsp | Self::R12 => 4,
            Self::Rbp | Self::R13 => 5,
            Self::Rsi | Self::R14 => 6,
            Self::Rdi | Self::R15 => 7,
        }
    }
    /// `true` si requiere prefijo REX.B (R8..R15).
    #[inline]
    pub const fn needs_rex_b(self) -> bool { (self as u8) >= 8 }
    /// `true` si requiere prefijo REX.R (en modrm.reg).
    #[inline]
    pub const fn needs_rex_r(self) -> bool { (self as u8) >= 8 }

    /// Nombre legible.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rax => "rax", Self::Rcx => "rcx", Self::Rdx => "rdx",
            Self::Rbx => "rbx", Self::Rsp => "rsp", Self::Rbp => "rbp",
            Self::Rsi => "rsi", Self::Rdi => "rdi",
            Self::R8 => "r8", Self::R9 => "r9", Self::R10 => "r10",
            Self::R11 => "r11", Self::R12 => "r12", Self::R13 => "r13",
            Self::R14 => "r14", Self::R15 => "r15",
        }
    }
}

/// Convierte índice de argumento (0..5) a registro.
#[inline]
pub const fn arg_reg(i: usize) -> Option<Reg> {
    ARG_REGS.get(i).copied()
}

/// Calcula el frame size para una función con `n_locals` locals de 8 bytes.
#[inline]
pub const fn frame_size(n_locals: u32) -> u32 {
    // 16-byte aligned, 16 extra para la dirección de retorno.
    let bytes = n_locals * 8;
    (bytes + 15) & !15
}
