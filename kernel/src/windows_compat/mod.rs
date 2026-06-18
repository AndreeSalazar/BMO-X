//! Windows Compatibility Layer for FastOS/BMO.
//!
//! This module provides transparent Win32 API compatibility, allowing
//! Windows PE applications to run on FastOS without modification.
//!
//! # Architecture
//!
//! PE apps import from standard Windows DLLs (kernel32.dll, user32.dll, etc.).
//! The PE devour loader resolves these imports to this compatibility layer,
//! which translates Win32 calls to BMO syscalls and barex functions.
//!
//! # Priority
//!
//! - P0: CRT (msvcrt) — malloc, printf, exit, SEH basics
//! - P1: kernel32 — process, memory, file, thread
//! - P2: user32 + gdi32 — windows, messages, drawing
//! - P3: shell32, advapi32, comctl32, ole32

#![allow(dead_code)]

pub mod api_map;

pub mod ntdll;
pub mod kernel32;
pub mod user32;
pub mod gdi32;
pub mod msvcrt;
pub mod advapi32;
pub mod shell32;
pub mod comctl32;
pub mod ole32;
pub mod seh;

/// Initialize the Windows compatibility layer.
pub fn init() {
    ntdll::init();
    crate::diag::info("wcompat", "Windows compatibility layer initialized");
    crate::diag::info_u64("wcompat", "total APIs mapped", api_map::TOTAL_MAPPED as u64);
}
