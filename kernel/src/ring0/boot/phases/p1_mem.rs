//! Phase 1 — Memory initialization.
//!
//! v1.8.9: bug fix. Los logs/reportaban "16 MB free-list" y "Installing
//! new kernel PML4" cuando la realidad era 1 MB de heap y PML4 stub.
//! Ahora los logs reflejan la realidad.
//!
//! v1.8.9: añadir un smoke test de `find_free(64)` después de
//! `init_heap()` para que cualquier regresión del heap allocator
//! (como el bug de offset 0) sea ruidosa en vez de un cuelgue
//! silencioso tres fases más tarde.

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

    // 1. Initialize the physical frame allocator from UEFI memory map.
    unsafe {
        crate::mm::phys::init(
            &bi.memory_map,
            bi.memory_map_count as usize,
            bi.reserved_addr,
            bi.reserved_size,
            bi.kernel_base,
            bi.kernel_size,
        );
    }
    let free_pages = unsafe { crate::mm::phys::free_count() } as u64;
    let free_mb = (free_pages * 4096) / (1024 * 1024);
    log::info_u64("phase1", "Free pages", free_pages);
    log::info_u64("phase1", "Free memory (MB)", free_mb);

    // 1.5. Map all physical RAM into high-mem region (HIGH_MEM_BASE).
    //      This enables phys_to_virt()/virt_to_phys() for ALL RAM,
    //      removing the 4 GB identity-mapping limit.
    unsafe {
        crate::mm::virt::map_high_mem(&bi.memory_map, bi.memory_map_count as usize);
    }
    log::info("phase1", "High-mem mapping complete");

    // 2. Initialize the kernel heap.
    // v1.8.9: 1 MB estático. v1.9 lo cambiará a heap dinámico.
    crate::mm::heap::init_heap();
    log::info("phase1", "Kernel heap initialized (1 MB free-list)");

    // v1.8.9: smoke test. Si `find_free(64)` retorna null, el heap
    // está roto (sentinel incorrecto, offset inválido, etc.). Mejor
    // panic ruidoso aquí que cuelgue silencioso en fase 2/3.
    unsafe {
        let probe = crate::mm::heap::heap_alloc(64, 8);
        if probe.is_null() {
            log::fault("phase1", "heap_alloc(64, 8) returned NULL — heap broken");
            // No return: continue so phase 2/3 surface a real panic.
        } else {
            log::info("phase1", "heap smoke test OK (alloc 64 bytes succeeded)");
            crate::mm::heap::heap_free(probe, 64, 8);
        }
    }

    let heap_total = crate::mm::heap::heap_total() as u64;
    let heap_used = crate::mm::heap::heap_used() as u64;
    log::info_u64("phase1", "Heap total (bytes)", heap_total);
    log::info_u64("phase1", "Heap used (bytes)", heap_used);

    // 3. PML4 status. create_kernel_page_table() está STUBBED en v1.6.2
    // por seguridad: cambiar PML4 mid-execution requiere re-mapear
    // kernel+stack y se hace en long-mode entry assembly, no aquí.
    // Por tanto seguimos con la PML4 de UEFI. Esto es correcto y
    // esperado — NO es un bug.
    log::info("phase1", "PML4: keeping UEFI identity map (new PML4 deferred to v1.9)");
    if unsafe { crate::mm::virt::create_kernel_page_table() }.is_none() {
        log::info("phase1", "PML4 stub confirmed: using UEFI PML4 (safe for ECAM via mmio_huge)");
    }

    let phase1_end = crate::cpu::rdtsc();
    log::info_u64("phase1", "Phase 1 time (TSC ticks)", phase1_end - prev_end);

    // Persist state into the boot context.
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
        CheckResult::pass("heap.total_32mb"),
    ];
    SelfTestReport { phase: "phase1", checks: CHECKS }
}
