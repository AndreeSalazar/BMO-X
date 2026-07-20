//! Calling convention — defines register usage, stack rules, and argument passing.
//!
//! Encodes the BMO ABI calling convention as Rust code (not just SPEC.md docs).
//! Language frontends and code generators query this to know which registers to use.

use crate::bmo_abi::primitives::{bx_u32, bx_u8};

/// Number of GPR (General Purpose Register) argument slots.
pub const GPR_ARG_COUNT: usize = 7;

/// Red zone size in bytes below RSP.
pub const RED_ZONE_BYTES: u32 = 256;

/// Required stack alignment before a call.
pub const STACK_ALIGN_BYTES: u32 = 64;

/// Calling convention variant.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    /// BMO native x86-64: 7 GPR args (RDI, RSI, RDX, RCX, R8, R9, R10), RAX:RDX return.
    /// Red zone: 256 bytes. Stack alignment: 64 bytes. No shadow space.
    BmoX86_64 = 0,

    /// BMO native ARM64 (AArch64): X0-X7 args, X0-X1 return.
    /// Reserved for future ARM64 port.
    BmoArm64 = 1,

    /// BMO native RISC-V (RV64GC): A0-A7 args, A0-A1 return.
    /// Reserved for future RISC-V port.
    BmoRiscV64 = 2,

    /// System V AMD64 ABI (Linux compatibility): RDI, RSI, RDX, RCX, R8, R9.
    /// Used for ELF binary compatibility shims.
    SystemVAmd64 = 3,
}

impl CallingConvention {
    /// Number of GPR argument registers.
    pub const fn gpr_arg_count(self) -> bx_u8 {
        match self {
            Self::BmoX86_64 => 7,
            Self::SystemVAmd64 => 6,
            Self::BmoArm64 | Self::BmoRiscV64 => 8,
        }
    }

    /// Returns the GPR registers used for arguments (as register names).
    pub fn arg_registers(self) -> &'static [&'static str] {
        match self {
            Self::BmoX86_64 => &["rdi", "rsi", "rdx", "rcx", "r8", "r9", "r10"],
            Self::SystemVAmd64 => &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
            Self::BmoArm64 => &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"],
            Self::BmoRiscV64 => &["a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7"],
        }
    }

    /// Return value registers.
    pub fn return_registers(self) -> &'static [&'static str] {
        match self {
            Self::BmoX86_64 | Self::SystemVAmd64 => &["rax", "rdx"],
            Self::BmoArm64 | Self::BmoRiscV64 => &["x0", "x1"],
        }
    }

    /// Stack alignment required before a call.
    pub const fn stack_align(self) -> bx_u32 {
        match self {
            Self::BmoX86_64 | Self::SystemVAmd64 => 64,
            Self::BmoArm64 | Self::BmoRiscV64 => 16,
        }
    }

    /// Red zone size (0 if none).
    pub const fn red_zone(self) -> bx_u32 {
        match self {
            Self::BmoX86_64 => 256,
            Self::SystemVAmd64 => 128,
            _ => 0,
        }
    }

    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Self::BmoX86_64 => "bmo-x86_64",
            Self::BmoArm64 => "bmo-arm64",
            Self::BmoRiscV64 => "bmo-riscv64",
            Self::SystemVAmd64 => "systemv-amd64",
        }
    }

    /// The default calling convention for the current architecture.
    #[cfg(target_arch = "x86_64")]
    pub const NATIVE: CallingConvention = CallingConvention::BmoX86_64;

    #[cfg(target_arch = "aarch64")]
    pub const NATIVE: CallingConvention = CallingConvention::BmoArm64;

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub const NATIVE: CallingConvention = CallingConvention::BmoRiscV64;
}

/// Scalar types that can be passed in a single GPR.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarKind {
    Void = 0,
    I8 = 1,
    I16 = 2,
    I32 = 3,
    I64 = 4,
    U8 = 5,
    U16 = 6,
    U32 = 7,
    U64 = 8,
    F32 = 9,
    F64 = 10,
    Pointer = 11,
    Bool = 12,
}

impl ScalarKind {
    pub const fn size(self) -> bx_u8 {
        match self {
            Self::Void => 0,
            Self::I8 | Self::U8 | Self::Bool => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 | Self::Pointer => 8,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Pointer => "ptr",
            Self::Bool => "bool",
        }
    }
}
