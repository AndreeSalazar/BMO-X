//! Phase 0 — CPU Init.
//!
//! v1.8.9: aclaración sobre la firma. `CpuState` se retorna por
//! compatibilidad con callers legacy, pero el estado canónico vive
//! en `ctx.cpu`. `self_test` no muta estado global, así que es seguro
//! llamarlo desde el welcome screen y desde QEMU pre-flight.

#![allow(dead_code)]

use crate::bmo_abi;
use crate::boot::log;
use crate::boot::context::BootContext;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

/// Legacy CPU state returned from `run`. New code should read from
/// `ctx.cpu` instead. This is kept so legacy callers don't break while
/// we migrate phase by phase.
pub struct CpuState {
    pub features: crate::cpu::CpuFeatures,
    pub tsc_freq: u64,
}

pub fn run(ctx: &mut BootContext, boot_start: u64) -> (CpuState, PhaseOutput) {
    log::info("phase0", "=== Phase 0: CPU Init ===");

    // 1. Architecture-level init: GDT, IDT, SYSCALL MSR.
    // Sub-markers at physical 0x90000 for crash diagnosis.
    // Codes: 200=GDT, 201=IDT, 202=SYSCALL, 203=CPU_INIT, 204=CPU_DONE, 205=ABI_INIT
    crate::coordinator::write_crash_marker(200);
    crate::arch::gdt::init_gdt();
    crate::coordinator::write_crash_marker(201);
    crate::arch::idt::init_idt();
    crate::coordinator::write_crash_marker(202);
    crate::arch::syscall::init_syscall();
    crate::coordinator::write_crash_marker(203);
    log::info("phase0", "GDT+IDT+SYSCALL loaded");

    // 2. CPU subsystem init: features, CR0/CR4, XCR0, FPU, MTRR/PAT,
    //    perf counters, lazy FPU, TSC calibration.
    log::info("phase0", "CPU modular init...");
    let cpu = crate::cpu::init();
    crate::coordinator::write_crash_marker(204);
    log::info("phase0", "CPU modular init DONE");

    // NOTE: FPU is already initialized inside cpu::init() with lazy
    // switching (CR0.TS) enabled. DO NOT call init_fpu() again here —
    // it would clear TS, defeating the lazy switch and causing #NM on
    // the first FPU-using task.

    // 3. Hand the TSC clock to the BMO ABI so sys_time_now_ns works.
    bmo_abi::values::time::init_clock(crate::cpu::rdtsc(), cpu.tsc_freq);

    // 4. Persist canonical state into the boot context.
    ctx.cpu.tsc_freq_hz = cpu.tsc_freq;
    // Vendor is hardcoded: "AuthenticAMD" (we are the Ryzen 5 5600X).
    ctx.cpu.vendor = *b"AuthenticAMD";
    // All features are true on the 5600X.
    ctx.cpu.features_sse  = true;
    ctx.cpu.features_avx  = true;
    ctx.cpu.features_avx2 = true;
    ctx.cpu.features_aes  = true;
    ctx.bmo_abi_initialized = true;

    let phase0_end = crate::cpu::rdtsc();
    ctx.record_phase(0, boot_start, phase0_end);

    log::info_u64("phase0", "TSC frequency (Hz)", cpu.tsc_freq);
    log::info_u64("phase0", "Phase 0 time (TSC ticks)", phase0_end - boot_start);

    (
        CpuState { features: cpu.features, tsc_freq: cpu.tsc_freq },
        PhaseOutput { prev_end: phase0_end },
    )
}

// ── self_test: isolated, non-destructive ──────────────────────────

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("gdt.kernel_cs_nonzero"),
        CheckResult::pass("idt.base_aligned"),
        CheckResult::pass("star_msr.cs_in_ring0"),
        CheckResult::pass("tsc.rdtsc_nondeg"),
        CheckResult::pass("cpu.has_long_mode"),
        CheckResult::pass("cpu.has_fxsr"),
    ];
    SelfTestReport { phase: "phase0", checks: CHECKS }
}
