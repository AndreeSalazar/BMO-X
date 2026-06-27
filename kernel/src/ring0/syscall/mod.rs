//! `syscall/` — Syscall ABI for FastOS.
//!
//! v1.8.8: skeleton. Centralizes the syscall numbers and dispatch
//! tables that the old `arch::system_call_dispatcher` implements.
//!
//! Future: this will be reorganized so each layer (Ring 0, BMO Core,
//! BMO GPU) has its own dispatch table, registered into a unified
//! dispatcher that `arch::system_call_dispatcher` consults.

pub mod numbers;
pub mod ring0;
pub mod bmo_core;
pub mod gpu;

// ── Ring 3 API stubs ─────────────────────────────────────────────
pub mod mmap;
pub mod file_ops;
pub mod signals;
