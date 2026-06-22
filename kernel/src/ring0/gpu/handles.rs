//! `gpu/handles.rs` — GPU resource handles.
//!
//! v1.8.8: skeleton. Will hold handle types for GPU resources (buffers,
//! textures, command queues, fences, semaphores, surfaces, etc.) once
//! the RDNA4 driver lands.

#![allow(dead_code)]

/// Opaque handle for a GPU buffer allocation.
pub type GpuBufferHandle = u64;

/// Opaque handle for a GPU texture allocation.
pub type GpuTextureHandle = u64;

/// Opaque handle for a GPU command queue.
pub type GpuQueueHandle = u64;

/// Opaque handle for a GPU fence.
pub type FenceId = u64;

/// Opaque handle for a GPU semaphore.
pub type SemaphoreId = u64;

/// Opaque handle for a GPU surface (swapchain image).
pub type SurfaceHandle = u64;

/// Opaque handle for a GPU pipeline (graphics or compute).
pub type PipelineHandle = u64;

/// Opaque handle for a GPU shader module.
pub type ShaderHandle = u64;

/// Flags for GPU memory allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuMemFlags(pub u32);

impl GpuMemFlags {
    /// Default: device-local VRAM.
    pub const DEVICE_LOCAL: Self = Self(0);
    /// Host-visible (CPU can map and write).
    pub const HOST_VISIBLE: Self = Self(1 << 0);
    /// Host-coherent (no manual flushing required).
    pub const HOST_COHERENT: Self = Self(1 << 1);
    /// CPU-GPU cached.
    pub const CACHED: Self = Self(1 << 2);
    /// Use write-combining (avoid L1/L2 cache, good for streaming).
    pub const WRITE_COMBINING: Self = Self(1 << 3);
}

/// GPU memory types (Vulkan-like).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuMemoryType {
    DeviceLocal,
    HostVisible,
    HostCached,
    /// Lazy-allocated memory (physical <= 4 GB on AMD for legacy DMA).
    Lazy,
}
