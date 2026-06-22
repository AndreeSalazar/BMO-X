//! BMO Runtime — Runtime layer for BMO programs.
//!
//! Wraps kernel BMO/FastOS services into high-level APIs for BMO
//! programs. All native, no external dependencies.
//!
//! ## Modules
//!
//! - `error` — Error codes and Result type
//! - `mem` — Memory management (pool allocator over bump)
//! - `proc` — Processes and threads (spawn, exit, wait)
//! - `io` — Serial and framebuffer I/O
//! - `fs` — Filesystem operations (read)
//! - `time` — Clock, sleep, timers

#![allow(dead_code)]

pub mod error;
pub mod mem;
pub mod proc;
pub mod io;
pub mod fs;
pub mod time;

/// Runtime version.
pub const RUNTIME_VERSION: (u8, u8, u8) = (0, 1, 0);

/// Initialize the BMO runtime.
pub fn init() {
    crate::cabina::info("bmo_rt", "BMO Runtime initialized");
}

