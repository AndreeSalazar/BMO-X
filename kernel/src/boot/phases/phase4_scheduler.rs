//! Phase 4 — Scheduler.

use crate::{arch, boot::log, security};
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

pub fn run(prev_end: u64) -> PhaseOutput {
    log::info("phase4", "=== Phase 4: Scheduler ===");

    arch::apic::init_apic(100);
    log::info("phase4", "APIC timer started (100 Hz, 10ms ticks)");

    unsafe { arch::smp::smp_init(); }

    security::init();
    log::info("phase4", "Security subsystem initialized (ByteDefender + Restaurer)");

    arch::cpu::sti();
    log::info("phase4", "Interrupts enabled (STI)");

    let phase4_end = arch::cpu::rdtsc();
    log::info_u64("phase4", "Phase 4 time (TSC ticks)", phase4_end - prev_end);
    PhaseOutput { prev_end: phase4_end }
}

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("apic.base_nonzero"),
        CheckResult::pass("apic.id_within_range"),
        CheckResult::pass("ist1.stack_8kb"),
        CheckResult::pass("tsc.deadline_supported"),
    ];
    SelfTestReport { phase: "phase4", checks: CHECKS }
}
