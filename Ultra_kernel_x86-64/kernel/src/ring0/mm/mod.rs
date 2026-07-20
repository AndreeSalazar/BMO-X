//! Memory management: physical frames (`phys`) + virtual address spaces (`vmm`).
//!
//! The physmap installed by `s2_mem` (physical 0..16 GiB mirrored at
//! `HIGH_MEM_BASE`) is the single mechanism Ring 0 uses to touch page-table
//! memory. No temporary mappings, no remap dances.

pub mod phys;
pub mod vmm;

/// Base of the direct physical map installed by `s2_mem`.
pub const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;
/// 4 KiB page.
pub const PAGE: u64 = 4096;

/// Physical address → kernel-virtual address via the physmap.
#[inline]
pub const fn phys_to_virt(phys: u64) -> u64 {
    phys + HIGH_MEM_BASE
}
