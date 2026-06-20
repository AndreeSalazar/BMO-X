//! Emisor multi-arquitectura para BMO Simple.
//! Agrupa y modulariza los codegen backends por carpetas de arquitectura.

pub mod x86_64;
pub mod aarch64;
pub mod riscv;
pub mod backend;

pub use x86_64::reg::Reg64;
pub use x86_64::encoder::Emitter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetArch {
    X86_64,
    Aarch64,
    Riscv64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRegister {
    X86_64(x86_64::reg::Reg64),
    Aarch64(aarch64::RegArm),
    Riscv64(riscv::RegRiscv),
}

impl TargetRegister {
    pub fn from_name(arch: TargetArch, name: &str) -> Option<Self> {
        match arch {
            TargetArch::X86_64 => x86_64::reg::Reg64::from_name(name).map(Self::X86_64),
            TargetArch::Aarch64 => aarch64::RegArm::from_name(name).map(Self::Aarch64),
            TargetArch::Riscv64 => riscv::RegRiscv::from_name(name).map(Self::Riscv64),
        }
    }
}

pub enum TargetEmitter {
    X86_64(x86_64::encoder::Emitter),
    Aarch64(aarch64::EmitterArm),
    Riscv64(riscv::EmitterRiscv),
}

impl TargetEmitter {
    pub fn new(arch: TargetArch) -> Self {
        match arch {
            TargetArch::X86_64 => Self::X86_64(x86_64::encoder::Emitter::new()),
            TargetArch::Aarch64 => Self::Aarch64(aarch64::EmitterArm::new()),
            TargetArch::Riscv64 => Self::Riscv64(riscv::EmitterRiscv::new()),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::X86_64(e) => &e.bytes,
            Self::Aarch64(e) => &e.bytes,
            Self::Riscv64(e) => &e.bytes,
        }
    }

    pub fn bytes_mut(&mut self) -> &mut alloc::vec::Vec<u8> {
        match self {
            Self::X86_64(e) => &mut e.bytes,
            Self::Aarch64(e) => &mut e.bytes,
            Self::Riscv64(e) => &mut e.bytes,
        }
    }
}

