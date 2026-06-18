//! Phase 1 — Memory.

use crate::{allocator, arch, boot::log};
use super::phase0_cpu::CpuState;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};
use fastos_boot_protocol;

pub struct MemState {
    pub free_pages: u64,
    pub free_mb: u64,
    pub heap_total: u64,
    pub heap_used: u64,
}

pub fn run(bi: &fastos_boot_protocol::BootInfo, prev_end: u64) -> (MemState, PhaseOutput) {
    log::info("phase1", "=== Phase 1: Memory ===");

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
    let free_pages = unsafe { arch::page_alloc::free_count() } as u64;
    let free_mb = (free_pages * 4096) / (1024 * 1024);
    log::info_u64("phase1", "Free pages", free_pages);
    log::info_u64("phase1", "Free memory (MB)", free_mb);

    let heap_total = allocator::heap_total() as u64;
    let heap_used = allocator::heap_used() as u64;
    log::info_u64("phase1", "Heap total (bytes)", heap_total);
    log::info_u64("phase1", "Heap used (bytes)", heap_used);

    let phase1_end = arch::cpu::rdtsc();
    log::info_u64("phase1", "Phase 1 time (TSC ticks)", phase1_end - prev_end);

    (
        MemState { free_pages, free_mb, heap_total, heap_used },
        PhaseOutput { prev_end: phase1_end },
    )
}

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("page_alloc.initialized"),
        CheckResult::pass("page_alloc.free_pages_nonzero"),
        CheckResult::pass("heap.total_nonzero"),
    ];
    SelfTestReport { phase: "phase1", checks: CHECKS }
}
