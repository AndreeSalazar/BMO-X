//! BMO kernel — entry point.
//!
//! The kernel is loaded at 0x400000 by `uefi_chain` layer 4 (ExitBootServices).
//! On entry, `rdi` holds a `*const BootContext` populated by the chain.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

pub mod info;
pub mod ring0;

// Re-export the kernel entry point as the public symbol.
// `_start` is `#[unsafe(no_mangle)]` in `ring0::core::entry`.
