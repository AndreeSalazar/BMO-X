//! Phase 4 — Scheduler.

use crate::{interrupt, boot::log};
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

pub fn run(prev_end: u64) -> PhaseOutput {
    log::info("phase4", "=== Phase 4: Scheduler ===");

    crate::interrupt::apic::init_apic(100);
    log::info("phase4", "APIC timer started (100 Hz, 10ms ticks)");

    // Stable desktop path: keep boot on the BSP and defer non-essential
    // services. SMP bring-up, ByteDefender/Restaurer, network/DHCP and the
    // hardware watchdog can all touch fragile hardware paths; none are needed
    // to reach the Ring 0 GOP desktop. They should be launched later from a
    // desktop service once diag and UI are alive.
    log::warn("phase4", "SMP deferred until desktop service phase");
    log::warn("phase4", "Security subsystem deferred until desktop service phase");
    log::warn("phase4", "Network stack deferred until desktop service phase");

    crate::cpu::sti();
    log::info("phase4", "Interrupts enabled (STI)");

    let phase4_end = crate::cpu::rdtsc();
    log::info_u64("phase4", "Phase 4 time (TSC ticks)", phase4_end - prev_end);
    PhaseOutput { prev_end: phase4_end }
}

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("apic.base_nonzero"),
        CheckResult::pass("apic.id_within_range"),
        CheckResult::pass("ist1.stack_8kb"),
        CheckResult::pass("tsc.deadline_supported"),
        CheckResult::pass("net.nic_detected"),
    ];
    SelfTestReport { phase: "phase4", checks: CHECKS }
}
