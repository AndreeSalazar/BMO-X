//! BMO Core — Windowing API + UI + Lang + FS + Desktop.
//!
//! BMO Core is the "intermediate layer" between Ring 0 (kernel privileged)
//! and Ring 3 (userland apps). It hosts the windowing API, desktop GUI,
//! languages (BMO), filesystem, and diagnostics.
//!
//! Unlike Ring 0, BMO Core doesn't require special privileges for its
//! logic (most runs with Ring 0 implicit in the kernel image). However,
//! its state is logically isolated: Ring 3 can only access BMO Core
//! via the 256 syscalls 0x100..0x1FF (BMO API v2.0).
//!
//! Submodules:
//!   bmo_api       — BMO API v2.0: 256 syscalls, window manager, paint compositor
//!   desktop       — Welcome + desktop Ring 0 supervisor
//!   ui            — Framebuffer primitives + 8x16 font
//!   diag          — Diagnostic overlay + events + telemetry
//!   gustos        — Audio system (FM synth, chimes, procedural tracks)
//!   bmo_abi       — BMO ABI primitives (handles, status, types)
//!   lang          — Languages: BMO (compiler) + runtime
//!   bef           — BEF binary devourer (PE/ELF/native)
//!   fs            — Filesystems: FAT32 + exFAT + ramdisk
//!
//! Contract with Ring 0:
//!   - BMO Core can call `crate::*` freely (same image).
//!   - Ring 0 exposes `crate::cpu::rdtsc`, `crate::cpu::busy_wait_ms` and
//!     legacy syscalls that BMO Core uses for timing.
//!
//! Contract with Ring 3 (see ../ring3/mod.rs):
//!   - BMO Core exposes 256 syscalls 0x100..0x1FF.
//!   - Ring 3 only sees #[repr(C)] types and stable fn signatures.

#![allow(dead_code)]
#![allow(static_mut_refs)]

#[path = "bmo_core.rs"]
pub mod coord;

pub mod bmo_api;
pub mod desktop;
pub mod ui;
pub mod diag;
pub mod gustos;
pub mod bmo_abi;
pub mod lang;
pub mod bef;
pub mod fs;
