//! Memory management: physical frames (`phys`) + virtual address spaces (`vmm`).
//!
//! The physmap installed by `s2_mem` (physical 0..16 GiB mirrored at
//! `HIGH_MEM_BASE`) is the single mechanism Ring 0 uses to touch page-table
//! memory. No temporary mappings, no remap dances.

pub mod phys;
pub mod vmm;

/// Base of the direct physical map installed by `s2_mem`.
pub const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;
/// Bytes of physical memory the `s2_mem` physmap mirrors at `HIGH_MEM_BASE`
/// (step 4 of its paging setup: 8192 × 2 MiB huge pages = 16 GiB). The frame
/// allocator MUST never hand out a frame at or above this address — the
/// kernel could not touch it through `phys_to_virt`. If s2_mem's mirror
/// grows (the 1 TiB path: size it from the memory map with 1 GiB pages),
/// update BOTH places together.
pub const PHYSMAP_SIZE: u64 = 0x4_0000_0000; // 16 GiB
/// 4 KiB page.
pub const PAGE: u64 = 4096;

/// Physical address → kernel-virtual address via the physmap.
#[inline]
pub const fn phys_to_virt(phys: u64) -> u64 {
    phys + HIGH_MEM_BASE
}
