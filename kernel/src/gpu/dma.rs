//! DMA Buffer Management for GPU Command Submission
//!
//! Allocates physically contiguous buffers in the 4MB-8MB identity-mapped region.
//! Used for pushbuffers (command streams), GPFIFO entries, and data transfers.

/// A GPU-accessible DMA buffer (physically contiguous, identity-mapped).
pub struct GpuDmaBuffer {
    /// Virtual address (= physical address due to identity mapping).
    pub virt: *mut u8,
    /// Physical address for GPU to access.
    pub phys: u64,
    /// Size in bytes.
    pub size: usize,
}

impl GpuDmaBuffer {
    /// Write a u32 at byte offset.
    #[inline]
    pub fn write_u32(&self, offset: usize, val: u32) {
        debug_assert!(offset + 4 <= self.size && offset % 4 == 0);
        unsafe {
            core::ptr::write_volatile(self.virt.add(offset) as *mut u32, val);
        }
    }

    /// Read a u32 at byte offset.
    #[inline]
    pub fn read_u32(&self, offset: usize) -> u32 {
        debug_assert!(offset + 4 <= self.size && offset % 4 == 0);
        unsafe {
            core::ptr::read_volatile(self.virt.add(offset) as *const u32)
        }
    }

    /// Zero the entire buffer.
    pub fn zero(&self) {
        unsafe { core::ptr::write_bytes(self.virt, 0, self.size); }
    }

    /// Get a u32 slice view of the buffer.
    pub fn as_u32_slice(&self) -> &[u32] {
        unsafe {
            core::slice::from_raw_parts(self.virt as *const u32, self.size / 4)
        }
    }
}

/// Simple bump allocator for GPU DMA buffers.
/// Uses the 4MB-8MB range (identity mapped by bootloader).
static mut GPU_DMA_NEXT: u64 = 0x0050_0000; // Start at 5MB (after platform DMA at 4MB)
const GPU_DMA_LIMIT: u64 = 0x0080_0000;     // 8MB ceiling

/// Allocate a GPU DMA buffer of `size` bytes (4KB aligned).
pub fn alloc_gpu_dma(size: usize) -> Option<GpuDmaBuffer> {
    unsafe {
        let aligned = (GPU_DMA_NEXT + 0xFFF) & !0xFFF; // 4KB align
        let end = aligned + size as u64;
        if end > GPU_DMA_LIMIT {
            return None;
        }
        GPU_DMA_NEXT = end;

        // Zero the buffer
        core::ptr::write_bytes(aligned as *mut u8, 0, size);

        Some(GpuDmaBuffer {
            virt: aligned as *mut u8,
            phys: aligned, // identity mapped
            size,
        })
    }
}

/// Report how much GPU DMA memory remains.
pub fn gpu_dma_remaining() -> usize {
    unsafe {
        (GPU_DMA_LIMIT - GPU_DMA_NEXT) as usize
    }
}
