//! `gpu/syscalls.rs` — GPU syscall interface.
//!
//! v1.8.8: stub. Will host the syscall ABI for GPU operations once
//! the RDNA4 driver and BMO GPU are wired together. The syscall numbers
//! are reserved in the `0x1E0..0x1FF` range (above BMO API v2.0).
//!
//! ## GPU syscall ABI (reserved)
//!
//! | nr     | name              | args                                |
//! |--------|-------------------|-------------------------------------|
//! | 0x1E0  | gpu_init          | () -> GpuDeviceId                   |
//! | 0x1E1  | gpu_alloc_vram    | (size, flags) -> GpuBufferHandle    |
//! | 0x1E2  | gpu_free_vram     | (handle) -> Result<()>              |
//! | 0x1E3  | gpu_submit        | (queue, cmd_ptr, cmd_len) -> FenceId|
//! | 0x1E4  | gpu_wait_fence    | (fence, timeout_ns) -> Result<()>   |
//! | 0x1E5  | gpu_create_swapchain | (surface_count, fmt) -> GpuQueueHandle |
//! | 0x1E6  | gpu_present       | (swapchain, image_idx) -> Result<()>|
//! | 0x1E7  | gpu_load_shader   | (bsf_ptr, bsf_len) -> ShaderHandle  |
//! | 0x1E8  | gpu_create_pipeline | (vs, fs, layout) -> PipelineHandle|
//! | 0x1E9  | gpu_bind_pipeline | (pipeline) -> Result<()>           |
//! | 0x1EA  | gpu_set_viewport  | (x, y, w, h) -> Result<()>          |
//! | 0x1EB  | gpu_set_scissor   | (x, y, w, h) -> Result<()>          |
//!
//! v1.8.8: these numbers are reserved but not implemented. They will
//! land alongside the BMO GPU phase.

#![allow(dead_code)]

/// Reserved syscall number for `gpu_init`.
pub const NR_GPU_INIT: u32 = 0x1E0;
/// Reserved syscall number for `gpu_alloc_vram`.
pub const NR_GPU_ALLOC_VRAM: u32 = 0x1E1;
/// Reserved syscall number for `gpu_free_vram`.
pub const NR_GPU_FREE_VRAM: u32 = 0x1E2;
/// Reserved syscall number for `gpu_submit`.
pub const NR_GPU_SUBMIT: u32 = 0x1E3;
/// Reserved syscall number for `gpu_wait_fence`.
pub const NR_GPU_WAIT_FENCE: u32 = 0x1E4;
/// Reserved syscall number for `gpu_create_swapchain`.
pub const NR_GPU_CREATE_SWAPCHAIN: u32 = 0x1E5;
/// Reserved syscall number for `gpu_present`.
pub const NR_GPU_PRESENT: u32 = 0x1E6;
/// Reserved syscall number for `gpu_load_shader`.
pub const NR_GPU_LOAD_SHADER: u32 = 0x1E7;
/// Reserved syscall number for `gpu_create_pipeline`.
pub const NR_GPU_CREATE_PIPELINE: u32 = 0x1E8;
/// Reserved syscall number for `gpu_bind_pipeline`.
pub const NR_GPU_BIND_PIPELINE: u32 = 0x1E9;
/// Reserved syscall number for `gpu_set_viewport`.
pub const NR_GPU_SET_VIEWPORT: u32 = 0x1EA;
/// Reserved syscall number for `gpu_set_scissor`.
pub const NR_GPU_SET_SCISSOR: u32 = 0x1EB;
/// Last reserved GPU syscall number (inclusive).
pub const NR_GPU_LAST: u32 = 0x1FF;

/// Returns true if the syscall number `nr` is in the reserved GPU range.
pub const fn is_gpu_syscall(nr: u32) -> bool {
    nr >= NR_GPU_INIT && nr <= NR_GPU_LAST
}

/// Placeholder for `gpu_init()`. v1.8.8: always returns "not supported"
/// since the GPU driver is not implemented yet.
pub fn gpu_init() -> u64 {
    0xFFFF_FFFF_FFFF_FFFF  // "invalid device id" sentinel
}

/// Placeholder for `gpu_alloc_vram()`. v1.8.8: returns 0 (invalid).
pub fn gpu_alloc_vram(_size: u64, _flags: u32) -> u64 {
    0
}

/// Placeholder for `gpu_submit()`. v1.8.8: returns 0.
pub fn gpu_submit(_queue: u64, _cmd_ptr: u64, _cmd_len: u64) -> u64 {
    0
}

/// Placeholder for `gpu_wait_fence()`. v1.8.8: returns 0xFFFF_FFFF
/// (timeout).
pub fn gpu_wait_fence(_fence: u64, _timeout_ns: u64) -> u64 {
    0xFFFF_FFFF
}
