//! `gpu/kernel_device.rs` — Minimal kernel device interface for the GPU.
//!
//! v1.8.8: skeleton. Will host the `GpuKernelDevice` trait that BMO GPU
//! uses to talk to the underlying vendor driver. Designed to be a
//! zero-overhead monomorphized interface, not a virtual call.

#![allow(dead_code)]

use super::handles::{GpuBufferHandle, GpuQueueHandle, FenceId, GpuMemFlags, GpuMemoryType};

/// Errors returned by GPU operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuError {
    /// The device is not initialized.
    NotInitialized,
    /// Out of memory.
    OutOfMemory,
    /// The handle is invalid.
    InvalidHandle,
    /// The operation timed out.
    Timeout,
    /// The feature is not supported on this GPU.
    NotSupported,
    /// The GPU is lost (hardware error).
    DeviceLost,
    /// Unknown error.
    Other,
}

/// Result type for GPU operations.
pub type GpuResult<T> = Result<T, GpuError>;

/// Opaque GPU device identifier.
pub type GpuDeviceId = u32;

/// Minimal GPU device interface (zero-cost via monomorphization).
///
/// BMO GPU calls these methods directly — there is no `dyn` trait,
/// so the compiler can inline and optimize aggressively. Each vendor
/// driver (e.g. `vendor::amd::gpu::rdna4`) provides its own concrete
/// implementation.
pub trait GpuKernelDevice {
    /// Allocate a buffer in GPU memory.
    fn alloc_buffer(&mut self, size: usize, flags: GpuMemFlags, mem_type: GpuMemoryType)
        -> GpuResult<GpuBufferHandle>;

    /// Free a buffer.
    fn free_buffer(&mut self, handle: GpuBufferHandle) -> GpuResult<()>;

    /// Submit a command buffer to a queue.
    fn submit(&mut self, queue: GpuQueueHandle, cmd_ptr: u64, cmd_len: usize)
        -> GpuResult<FenceId>;

    /// Wait for a fence to be signaled (busy-wait with timeout in ns).
    fn wait_fence(&mut self, fence: FenceId, timeout_ns: u64) -> GpuResult<()>;

    /// Present the next swapchain image to the display.
    fn present(&mut self, swapchain: GpuQueueHandle, image_index: u32) -> GpuResult<()>;

    /// Get the device vendor ID (e.g. 0x1002 for AMD).
    fn vendor_id(&self) -> u16;

    /// Get the device product ID (e.g. 0x7480 for RX 9060 XT).
    fn device_id(&self) -> u16;

    /// Get the device name (null-terminated, max 64 chars).
    fn device_name(&self) -> &'static str;

    /// Get the total VRAM size in bytes.
    fn vram_size(&self) -> u64;
}

/// Stub device: returns `NotInitialized` for all operations.
/// v1.8.8: this is the default device when no GPU driver is active.
pub struct NullGpuDevice;

impl GpuKernelDevice for NullGpuDevice {
    fn alloc_buffer(&mut self, _size: usize, _flags: GpuMemFlags, _mem_type: GpuMemoryType) -> GpuResult<GpuBufferHandle> {
        Err(GpuError::NotInitialized)
    }
    fn free_buffer(&mut self, _handle: GpuBufferHandle) -> GpuResult<()> {
        Err(GpuError::NotInitialized)
    }
    fn submit(&mut self, _queue: GpuQueueHandle, _cmd_ptr: u64, _cmd_len: usize) -> GpuResult<FenceId> {
        Err(GpuError::NotInitialized)
    }
    fn wait_fence(&mut self, _fence: FenceId, _timeout_ns: u64) -> GpuResult<()> {
        Err(GpuError::NotInitialized)
    }
    fn present(&mut self, _swapchain: GpuQueueHandle, _image_index: u32) -> GpuResult<()> {
        Err(GpuError::NotInitialized)
    }
    fn vendor_id(&self) -> u16 { 0xFFFF }
    fn device_id(&self) -> u16 { 0xFFFF }
    fn device_name(&self) -> &'static str { "NullGpuDevice (no GPU driver active)" }
    fn vram_size(&self) -> u64 { 0 }
}
