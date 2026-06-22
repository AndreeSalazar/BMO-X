//! `syscall/gpu.rs` — BMO GPU syscall table (0x1E0..=0x1FF).
//!
//! v1.8.8: skeleton. Hosts the dispatch table for the GPU syscalls
//! (gpu_init, gpu_alloc_vram, gpu_submit, etc.). The actual
//! implementations will live in `crate::gpu::syscalls`.

#![allow(dead_code)]

use super::numbers::is_bmo_gpu;
use crate::gpu::syscalls::*;

/// Returns true if the syscall number is a BMO GPU syscall.
pub const fn is_bmo_gpu_syscall(nr: u32) -> bool {
    is_bmo_gpu(nr)
}

/// Dispatch a BMO GPU syscall. v1.8.8: all return "not supported".
pub fn dispatch(nr: u32, _a0: u64, _a1: u64, _a2: u64, _a3: u64) -> u64 {
    match nr {
        NR_GPU_INIT => gpu_init(),
        NR_GPU_ALLOC_VRAM => gpu_alloc_vram(_a0, _a1 as u32),
        NR_GPU_FREE_VRAM => 0,        // TODO
        NR_GPU_SUBMIT => gpu_submit(_a0, _a1, _a2),
        NR_GPU_WAIT_FENCE => gpu_wait_fence(_a0, _a1),
        _ => u64::MAX,  // not implemented
    }
}
