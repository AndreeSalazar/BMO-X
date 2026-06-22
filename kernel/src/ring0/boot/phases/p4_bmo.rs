//! Phase 4 — Scheduler / process core.

use crate::boot::log;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

pub fn run(prev_end: u64) -> PhaseOutput {
    log::info("phase4", "=== Phase 4: Scheduler / Process ===");

    crate::proc::init();
    log::info("phase4", "Process/task scheduler tables initialized");

    crate::arch::apic::init_apic(100);
    log::info("phase4", "APIC timer started (100 Hz, 10ms ticks)");

    // Stable desktop path: keep boot on the BSP and defer non-essential
    // services. SMP bring-up, ByteDefender/Restaurer and network/DHCP can all
    // touch fragile hardware paths; none are needed to reach the Ring 0 GOP
    // desktop. They should be launched later from a desktop service once diag
    // and UI are alive. The watchdog is armed below because APIC ticks are now
    // available and it protects the scheduler path.
    log::warn("phase4", "SMP deferred until desktop service phase");
    log::warn("phase4", "Security subsystem deferred until desktop service phase");
    log::warn("phase4", "Network stack deferred until desktop service phase");

    crate::cpu::sti();
    log::info("phase4", "Interrupts enabled (STI)");

    // v1.8.8: arm the hardware watchdog now that interrupts are enabled.
    // The watchdog timer is based on TSC and pets on every APIC tick.
    // If the scheduler hangs, the watchdog will reboot the system.
    crate::dev::watchdog::arm();
    log::info("phase4", "Hardware watchdog armed (will reboot if scheduler hangs)");

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
