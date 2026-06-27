//! `syscall/gpu.rs` — BMO GPU syscall table (0x1E0..=0x1FF).
//!
//! TEMPORAL: GPU module removed. All syscalls return "not supported".

#![allow(dead_code)]

use super::numbers::is_bmo_gpu;

pub const fn is_bmo_gpu_syscall(nr: u32) -> bool {
    is_bmo_gpu(nr)
}

pub fn dispatch(nr: u32, _a0: u64, _a1: u64, _a2: u64, _a3: u64) -> u64 {
    let _ = (nr, _a0, _a1, _a2, _a3);
    u64::MAX
}
