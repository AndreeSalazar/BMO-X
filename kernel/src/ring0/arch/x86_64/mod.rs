//! `arch/x86_64/` — x86-64 architecture mechanism (no vendor knowledge).
//!
//! v1.8.8: re-exports the existing `arch::gdt`, `arch::idt`,
//! `arch::apic`, `arch::ctx`, `arch::syscall` for the new path.
//!
//! This module knows about x86-64 but NOT about AMD vs Intel vs ARM.
//! All vendor-specific code lives in `vendor/`.

pub use crate::arch::gdt;
pub use crate::arch::idt;
pub use crate::arch::apic;
pub use crate::arch::ctx;
pub use crate::arch::syscall;
