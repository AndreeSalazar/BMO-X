//! Boot subsystem — early boot infrastructure.
//!
//! v1.1.0: Reorganized around `BootContext` (see `context.rs`).
//!
//! This module owns everything that runs before the desktop subsystem is up:
//!
//!   - `context` — DI container shared across phases
//!   - `log`     — single-path diagnostic + serial + visual logger
//!   - `visual`  — minimal GOP framebuffer text overlay for the first seconds
//!   - `serial`  — small number-formatter helpers used during boot
//!   - `phases`  — one sub-module per boot phase, called in order from `main`
//!
//! After Phase 5 returns (i.e. the welcome screen blocks), this module's
//! utilities are no longer the primary log path — `desktop` and `ui::console`
//! take over. `boot::log::info/warn/fault` remain available for any later
//! kernel subsystem that wants a uniform boot-style log.

pub mod context;
pub mod log;
pub mod serial;
pub mod visual;

pub mod phases;

// Re-export BootContext at the crate root for ergonomic call sites
pub use context::BootContext;

// Re-export boot serial helpers at `crate::boot::*` for legacy call sites
// that historically imported `serial_hex` / `serial_u32` from `crate::main`.
// New code should use `crate::boot::serial::hex` / `u32_dec`.
pub use serial::hex   as serial_hex;
