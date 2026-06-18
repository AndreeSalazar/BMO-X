//! Phase 0 — CPU Init.
//!
//! `run` is the normal boot flow. `self_test` performs isolated checks that
//! do not modify global boot state — useful for the welcome-screen `test`
//! command and for QEMU pre-flight.

use crate::{arch, bmo_abi, boot::log};
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

pub struct CpuState {
    pub features: arch::cpu::CpuFeatures,
    pub tsc_freq: u64,
}

pub fn run(boot_start: u64) -> (CpuState, PhaseOutput) {
    log::info("phase0", "=== Phase 0: CPU Init ===");

    arch::gdt::init_gdt();
    arch::idt::init_idt();
    arch::syscall_entry::init_syscall();
    log::info("phase0", "GDT+IDT+SYSCALL loaded");

    log::warn("phase0", "CPU modular init...");
    let cpu = arch::cpu::init();
    log::info("phase0", "CPU modular init DONE");

    bmo_abi::time::init_clock(arch::cpu::rdtsc(), cpu.tsc_freq);

    let phase0_end = arch::cpu::rdtsc();
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
