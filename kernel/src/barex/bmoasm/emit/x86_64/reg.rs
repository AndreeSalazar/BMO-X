//! Registros x86-64. Mapeo a los códigos REX/ModR/M.

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg64 {
    Rax = 0,  Rcx = 1,  Rdx = 2,  Rbx = 3,
    Rsp = 4,  Rbp = 5,  Rsi = 6,  Rdi = 7,
    R8  = 8,  R9  = 9,  R10 = 10, R11 = 11,
    R12 = 12, R13 = 13, R14 = 14, R15 = 15,
}

impl Reg64 {
    #[inline(always)]
    pub const fn code(self) -> u8 { self as u8 }

    /// Bit REX.B / REX.R necesario para r8..r15.
    #[inline(always)]
    pub const fn needs_rex(self) -> bool { (self as u8) >= 8 }

    /// Parsea un nombre textual: `"rax"`, `"r10"`, etc.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "rax" => Some(Self::Rax), "rcx" => Some(Self::Rcx),
            "rdx" => Some(Self::Rdx), "rbx" => Some(Self::Rbx),
            "rsp" => Some(Self::Rsp), "rbp" => Some(Self::Rbp),
            "rsi" => Some(Self::Rsi), "rdi" => Some(Self::Rdi),
            "r8"  => Some(Self::R8),  "r9"  => Some(Self::R9),
            "r10" => Some(Self::R10), "r11" => Some(Self::R11),
            "r12" => Some(Self::R12), "r13" => Some(Self::R13),
            "r14" => Some(Self::R14), "r15" => Some(Self::R15),
            _ => None,
        }
    }
}

/// Registros de argumento según BMO ABI (7 GPR int).
///
/// Orden: `RDI RSI RDX R10 R8 R9 RAX_extra`.
pub const BMO_ARG_REGS: [Reg64; 7] = [
    Reg64::Rdi, Reg64::Rsi, Reg64::Rdx,
    Reg64::R10, Reg64::R8,  Reg64::R9,
    Reg64::Rax,
];
