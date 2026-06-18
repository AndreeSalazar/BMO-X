//! Phase 1 — Memory.
//!
//! Initialises the page allocator from the UEFI memory map, validates the
//! BMO kernel heap, and reports free/used memory. After this phase returns,
//! `alloc::vec::Vec`, `String`, and friends work normally.
//!
//! Returns a snapshot of memory state for later phases and the desktop banner.

use crate::{allocator, arch, boot::log};
use fastos_boot_protocol;
use super::phase0_cpu::CpuState;

pub struct MemState {
    pub free_pages: u64,
    pub free_mb: u64,
    pub heap_total: u64,
    pub heap_used: u64,
}

pub fn run(bi: &fastos_boot_protocol::BootInfo, prev_end: u64) -> (MemState, u64) {
    log::info("phase1", "=== Phase 1: Memory ===");
    crate::boot::visual::log("phase1", "=== Phase 1: Memory ===",
        crate::boot::visual::color::HEADER);

    if bi.memory_map_count == 0 {
        log::fault("phase1", "UEFI memory map is empty");
    }
    log::info_u64("phase1", "UEFI memory map entries", bi.memory_map_count as u64);

    unsafe {
        arch::page_alloc::init(
            &bi.memory_map,
            bi.memory_map_count as usize,
            bi.gsp_addr,
            bi.gsp_size,
            bi.kernel_base,
            bi.kernel_size,
        );
    }
    let free_pages = unsafe { arch::page_alloc::free_count() };
    let free_mb = (free_pages * 4096) / (1024 * 1024);
    log::info_u64("phase1", "Free pages", free_pages as u64);
    log::info_u64("phase1", "Free memory (MB)", free_mb as u64);

    let heap_total = allocator::heap_total() as u64;
    let heap_used = allocator::heap_used() as u64;
    log::info_u64("phase1", "Heap total (bytes)", heap_total);
    log::info_u64("phase1", "Heap used (bytes)", heap_used);

    let phase1_end = arch::cpu::rdtsc();
    log::info_u64("phase1", "Phase 1 time (TSC ticks)", phase1_end - prev_end);

    (
        MemState { free_pages, free_mb, heap_total, heap_used },
        phase1_end,
    )
}
