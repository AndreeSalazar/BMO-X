//! Phase 0 — CPU Init.
//!
//! Order is fixed and intentional:
//!   1. GDT + IDT + SYSCALL entry — must be live before any fault can happen
//!   2. Modular CPU init (features, CR, XCR, FPU, MTRR, PAT, perf, TSC)
//!   3. BMO ABI clock init — depends on calibrated TSC frequency
//!
//! After this phase returns, the kernel can safely service #GP, #PF, #DF, #NMI.
//!
//! Returns the calibrated CPU state so callers can read TSC frequency and
//! feature flags without re-querying.

use crate::{arch, bmo_abi, boot::log};

pub struct CpuState {
    pub features: arch::cpu::CpuFeatures,
    pub tsc_freq: u64,
}

pub fn run(boot_start: u64) -> (CpuState, u64) {
    log::info("phase0", "=== Phase 0: CPU Init ===");
    crate::boot::visual::log("phase0", "=== Phase 0: CPU Init ===",
        crate::boot::visual::color::HEADER);

    arch::gdt::init_gdt();
    arch::idt::init_idt();
    arch::syscall_entry::init_syscall();
    crate::boot::visual::log("phase0", "GDT+IDT+SYSCALL loaded",
        crate::boot::visual::color::OK);

    crate::boot::visual::log("phase0", "CPU modular init...",
        crate::boot::visual::color::WARN);
    let cpu = arch::cpu::init();
    crate::boot::visual::log("phase0", "CPU modular init DONE",
        crate::boot::visual::color::OK);

    bmo_abi::time::init_clock(arch::cpu::rdtsc(), cpu.tsc_freq);

    let phase0_end = arch::cpu::rdtsc();
    log::info_u64("phase0", "TSC frequency (Hz)", cpu.tsc_freq);
    log::info_u64("phase0", "Phase 0 time (TSC ticks)", phase0_end - boot_start);

    (
        CpuState { features: cpu.features, tsc_freq: cpu.tsc_freq },
        phase0_end,
    )
}
