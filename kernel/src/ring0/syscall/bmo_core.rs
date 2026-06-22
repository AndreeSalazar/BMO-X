//! `syscall/bmo_core.rs` — BMO Core / BMO API v2 syscall table.
//!
//! v1.8.8: skeleton. Hosts the dispatch table for the 256 BMO API
//! syscalls (0x100..=0x1FF, of which 0x100..=0x1CF are the API itself
//! and 0x1E0..=0x1FF are reserved for BMO GPU).

#![allow(dead_code)]

use super::numbers::is_bmo_api_v2;

/// Returns true if the syscall number is a BMO API v2 syscall.
pub const fn is_bmo_core_syscall(nr: u32) -> bool {
    is_bmo_api_v2(nr)
}
