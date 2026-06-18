//! Phase 4 — Scheduler.
//!
//! Starts the APIC timer at 100 Hz, brings up Application Processors, and
//! initialises the security subsystem. Interrupts are enabled at the end of
//! this phase. After it returns, the system is fully preemptive.

use crate::{arch, boot::log, security};

pub fn run(prev_end: u64) -> u64 {
    log::info("phase4", "=== Phase 4: Scheduler ===");
    crate::boot::visual::log("phase4", "=== Phase 4: Scheduler ===",
        crate::boot::visual::color::HEADER);

    arch::apic::init_apic(100);
    log::info("phase4", "APIC timer started (100 Hz, 10ms ticks)");

    unsafe { arch::smp::smp_init(); }

    security::init();
    log::info("phase4", "Security subsystem initialized (ByteDefender + Restaurer)");

    arch::cpu::sti();
    log::info("phase4", "Interrupts enabled (STI)");

    let phase4_end = arch::cpu::rdtsc();
    log::info_u64("phase4", "Phase 4 time (TSC ticks)", phase4_end - prev_end);
    phase4_end
}
