//! Boot subsystem — early boot infrastructure.
//!
//! v1.7.5: Reorganized into phases and a typed context.
//!
//! This module owns everything that runs before the desktop subsystem is up:
//!
//!   - `info`    — BootInfo shared from bootloader (was `boot_info.rs`)
//!   - `context` — DI container shared across phases
//!   - `log`     — single-path diagnostic + serial + visual logger
//!   - `visual`  — minimal GOP framebuffer text overlay for the first seconds
//!   - `serial`  — small number-formatter helpers used during boot
//!   - `phases`  — one sub-module per boot phase, called in order from `main`

pub mod info;
pub mod context;
pub mod log;
pub mod serial;
pub mod visual;

pub mod phases;

// Re-export BootContext at the crate root for ergonomic call sites
pub use context::BootContext;

// Re-export boot serial helpers at `crate::boot::*` for legacy call sites.
pub use serial::hex as serial_hex;
pub use serial::u32_dec;
pub use serial::u64_dec;
