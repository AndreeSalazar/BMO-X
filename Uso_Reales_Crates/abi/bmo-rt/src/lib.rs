//! `userland_ring3` — the BMO userland standard library.
//!
//! Provides:
//! - Single syscall dispatch point `bmo_syscall` + all BMO syscall wrappers
//! - Heap allocator (`malloc`, `free`, `calloc`, `realloc`)
//! - String library (`memcpy`, `strlen`, `strcmp`, `strdup`, ...)
//! - Formatted output (`printf`, `sprintf`, `snprintf` with %d, %x, %s...)
//! - C runtime startup (`_start` → `main` → `exit`)
//!
//! # For C/COBOL
//!
//! Frontends generate `call` relocations to functions exported in BMO.toml.
//!
//! # For Rust
//!
//! Use `#[global_allocator]` via the heap module.

#![no_std]
#![allow(static_mut_refs)]

#[cfg(test)]
extern crate std;

pub mod syscall;
pub mod heap;
pub mod string;
pub mod fmt;
pub mod crt0;
pub mod input;

mod init;
pub mod ffi;
