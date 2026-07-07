//! `userland_ring3` â€” the BMO userland standard library.
//!
//! Provides the single syscall dispatch point `bmo_syscall`, plus
//! all BMO syscall wrappers, a heap allocator (`malloc`/`free`),
//! and language-agnostic ABI functions for C, COBOL, and Rust programs.
//!
//! # For C/COBOL
//!
//! The `BMO.toml` manifest exports these functions so frontends
//! generate `call` relocations instead of inline `syscall` instructions.
//!
//! # For Rust
//!
//! Use `#[global_allocator]` via the provided allocator, and call
//! convenience wrappers like `userland_ring3::syscall::mem_alloc`.

#![no_std]

#[cfg(test)]
extern crate std;

pub mod syscall;
pub mod heap;

mod init;
