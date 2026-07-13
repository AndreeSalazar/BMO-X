//! Virtual memory — stub for the Ring 0 base.
//!
//! The full paging setup (PML4, higher-half map, kernel page tables)
//! is implemented in `stage2_mm` of the boot chain, so by the time
//! the kernel runs the address space is already configured. This
//! module exists so the rest of the kernel can compile, but the
//! real logic lives upstream.

use super::types::MemoryEntry;

/// Stub: identity map of low memory. Returns Ok(()) always.
pub fn map_high_mem(_entries: &[MemoryEntry], _count: usize) -> Result<(), &'static str> {
    Ok(())
}

/// Stub: map a 2 MB MMIO region. Always succeeds.
pub fn map_kernel_mmio_huge(_phys: u64, _virt: u64, _size: usize) -> Result<(), &'static str> {
    Ok(())
}

/// Higher-half base address (matches `stage2_mm` layout).
pub const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;

pub fn phys_to_virt(phys: u64) -> u64 { phys + HIGH_MEM_BASE }
pub fn virt_to_phys(virt: u64) -> u64 { virt - HIGH_MEM_BASE }
