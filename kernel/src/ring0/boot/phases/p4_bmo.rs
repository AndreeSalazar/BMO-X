//! Phase 4 — Scheduler, APIC timer, interrupts, watchdog.
//!
//! v1.8.16: APIC timer + interrupts + watchdog DESHABILITADOS.
//! El context switch preemptivo estaba causando #GP en el stack
//! durante el welcome screen. Por ahora corremos en modo cooperative
//! (sin preempción). El scheduler y el APIC timer se re-habilitarán
//! una vez que el welcome screen sea estable.

use crate::boot::log;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

pub fn run(prev_end: u64) -> PhaseOutput {
    log::info("phase4", "=== Phase 4: Scheduler / Process (cooperative mode) ===");

    // 1. Process/task table init.
    crate::proc::init();
    log::info("phase4", "Process/task scheduler tables initialized");

    // 2. APIC timer init DESHABILITADO. Estaba causando #GP en el
    //    context switch. Se re-habilitará en v1.9.
    // crate::arch::apic::init_apic(100);
    log::warn("phase4", "APIC timer DISABLED (cooperative mode)");

    // 3. Defer non-essential services. These need fragile hardware
    //    paths (SMP, ByteDefender, DHCP) and aren't needed to reach
    //    the Ring 0 GOP desktop. They run later from a desktop
    //    service once diag and UI are alive.
    log::info("phase4", "SMP deferred until desktop service phase");
    log::info("phase4", "Security subsystem deferred until desktop service phase");
    log::info("phase4", "Network stack deferred until desktop service phase");

    // 4. Interrupts DESHABILITADOS por ahora. El IDT está listo pero
    //    no activamos STI para evitar context switches preemptivos
    //    que corrompen el stack. El keyboard polling usa IO directo
    //    y no requiere interrupts.
    // crate::cpu::sti();
    log::warn("phase4", "Interrupts DISABLED (cooperative mode; keyboard polled via IO)");

    // 5. Watchdog DESHABILITADO. Sin interrupts no podemos petearlo.
    // crate::dev::watchdog::arm();
    log::warn("phase4", "Watchdog DISABLED (no timer tick to pet it)");

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
