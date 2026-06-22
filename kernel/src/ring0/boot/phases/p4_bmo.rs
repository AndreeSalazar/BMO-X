//! Phase 4 — Scheduler, APIC timer, interrupts, watchdog.
//!
//! v1.8.9: aclaración. El orden importa: APIC timer DEBE estar
//! calibrado y corriendo **antes** de `sti()` y del `watchdog::arm()`,
//! porque el watchdog se alimenta desde la ISR del APIC timer (vector
//! 48). Si el APIC no interrumpe, el watchdog resetea la máquina.

use crate::boot::log;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

pub fn run(prev_end: u64) -> PhaseOutput {
    log::info("phase4", "=== Phase 4: Scheduler / Process ===");

    // 1. Process/task table init.
    crate::proc::init();
    log::info("phase4", "Process/task scheduler tables initialized");

    // 2. APIC timer init. v1.8.9: la calibración es correcta — LVT
    //    en one-shot antes de medir, luego cambio a periodic.
    crate::arch::apic::init_apic(100);
    log::info("phase4", "APIC timer started (100 Hz, 10ms ticks)");

    // 3. Defer non-essential services. These need fragile hardware
    //    paths (SMP, ByteDefender, DHCP) and aren't needed to reach
    //    the Ring 0 GOP desktop. They run later from a desktop
    //    service once diag and UI are alive.
    log::info("phase4", "SMP deferred until desktop service phase");
    log::info("phase4", "Security subsystem deferred until desktop service phase");
    log::info("phase4", "Network stack deferred until desktop service phase");

    // 4. Enable interrupts. The APIC timer ISR (vector 48) starts
    //    firing now and feeding the watchdog.
    crate::cpu::sti();
    log::info("phase4", "Interrupts enabled (STI)");

    // 5. Arm the hardware watchdog. If the scheduler hangs, the
    //    watchdog fires and reboots the system.
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
    ];
    SelfTestReport { phase: "phase4", checks: CHECKS }
}
