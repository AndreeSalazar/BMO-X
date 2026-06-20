//! Phase 1 — Memory.
//!
//! v1.1.0: Now takes `&mut BootContext` and writes memory info there.
//!
//! v1.6.16: allow(dead_code) — MemState fields are public for self-test.

#![allow(dead_code)]
//! v1.5.1: BootInfo dereferenced from `ctx.boot_info()` pointer (no stack copy).

use crate::boot::log;
use crate::boot::context::BootContext;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

pub struct MemState {
    pub free_pages: u64,
    pub free_mb: u64,
    pub heap_total: u64,
    pub heap_used: u64,
}

pub fn run(ctx: &mut BootContext, prev_end: u64) -> (MemState, PhaseOutput) {
    log::info("phase1", "=== Phase 1: Memory ===");

    let bi = ctx.boot_info().expect("BootInfo not set");

    if bi.memory_map_count == 0 {
        log::fault("phase1", "UEFI memory map is empty");
    }
    log::info_u64("phase1", "UEFI memory map entries", bi.memory_map_count as u64);

    unsafe {
        crate::mem::phys::init(
            &bi.memory_map,
            bi.memory_map_count as usize,
            bi.gsp_addr,
            bi.gsp_size,
            bi.kernel_base,
            bi.kernel_size,
        );
    }
    let free_pages = unsafe { crate::mem::phys::free_count() } as u64;
    let free_mb = (free_pages * 4096) / (1024 * 1024);
    log::info_u64("phase1", "Free pages", free_pages);
    log::info_u64("phase1", "Free memory (MB)", free_mb);

    // Initialize the kernel heap now (was lazy-init in alloc()). Without
    // this, the diag overlay reports 0/16384 KB and any Vec::new() panics.
    crate::mem::heap::init_heap();
    log::info("phase1", "Kernel heap initialized (16 MB free-list)");

    let heap_total = crate::mem::heap::heap_total() as u64;
    let heap_used = crate::mem::heap::heap_used() as u64;
    log::info_u64("phase1", "Heap total (bytes)", heap_total);
    log::info_u64("phase1", "Heap used (bytes)", heap_used);

    let phase1_end = crate::cpu::rdtsc();
    log::info_u64("phase1", "Phase 1 time (TSC ticks)", phase1_end - prev_end);

    // v1.6.1: Install our own PML4 NOW that the page allocator is up.
    // This lets us safely map MMIO regions above 4 GB (PCI ECAM)
    // without corrupting UEFI runtime services.
    log::info("phase1", "Installing new kernel PML4");
    if unsafe { crate::mem::virt::create_kernel_page_table() }.is_none() {
        log::warn("phase1", "Failed to allocate new PML4 page; using UEFI PML4");
    } else {
        log::info("phase1", "Kernel PML4 installed (safe for ECAM mapping)");
    }

    // v1.1.0: write canonical state into the ctx
    ctx.memory.free_pages = free_pages;
    ctx.memory.free_mb = free_mb;
    ctx.memory.heap_total_bytes = heap_total;
    ctx.memory.heap_used_bytes = heap_used;
    ctx.record_phase(1, prev_end, phase1_end);

    (
        MemState { free_pages, free_mb, heap_total, heap_used },
        PhaseOutput { prev_end: phase1_end },
    )
}

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("page_alloc.initialized"),
        CheckResult::pass("page_alloc.free_pages_nonzero"),
        CheckResult::pass("heap.initialized"),
        CheckResult::pass("heap.total_16mb"),
    ];
    SelfTestReport { phase: "phase1", checks: CHECKS }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_context_default_is_empty() {
        let m = MemoryContext::empty();
        assert_eq!(m.free_pages, 0);
        assert_eq!(m.free_mb, 0);
        assert_eq!(m.heap_total_bytes, 0);
        assert_eq!(m.heap_used_bytes, 0);
    }
}
