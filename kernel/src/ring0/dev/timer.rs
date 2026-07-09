//! Timer Manager (Ring 0 HAL).
//!
//! Manages all hardware timers and provides unified time APIs:
//!   - HPET: High Precision Event Timer (preferred, replaces PIT)
//!   - Timer wheel: Dynamic timeout management for kernel
//!   - Timestamp: High-resolution time abstraction (ns precision)
//!
//! Architecture:
//!   - HPET provides the global reference counter (nanosecond precision)
//!   - Timer wheel manages thousands of concurrent timeouts efficiently
//!   - Timestamp layer unifies TSC + HPET + APIC timer into one API
//!
//! Init order:
//!   1. TSC calibrated in phase0 (already done)
//!   2. HPET detected from ACPI, MMIO mapped
//!   3. Timer wheel initialized with HPET as clock source
//!   4. All `sleep()` / `timeout()` calls go through timer wheel


/// Timer source priority (best to worst).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerSource {
    Hpet,
    ApicTimer,
    Tsc,
    Pit,
}

/// Global timer state.
static mut CURRENT_SOURCE: TimerSource = TimerSource::Tsc;

/// Initialize the timer subsystem.
pub fn init() {
    crate::dev::console::serial_write("[timer] initializing\n");

    // Try HPET first
    if super::hpet::is_available() {
        super::hpet::init();
        unsafe { CURRENT_SOURCE = TimerSource::Hpet; }
        crate::dev::console::serial_write("[timer] using HPET as clock source\n");
    } else {
        crate::dev::console::serial_write("[timer] HPET not available, using TSC\n");
    }

    // Init timer wheel
    super::timer_wheel::init();
    crate::dev::console::serial_write("[timer] timer wheel initialized\n");
}

/// Get current timestamp in nanoseconds.
/// Also updates the vDSO page for Ring 3 fast access.
pub fn now_ns() -> u64 {
    crate::vdso::tick();
    match unsafe { CURRENT_SOURCE } {
        TimerSource::Hpet => super::hpet::now_ns(),
        TimerSource::Tsc => super::timestamp::tsc_to_ns(crate::cpu::rdtsc()),
        _ => super::timestamp::tsc_to_ns(crate::cpu::rdtsc()),
    }
}

/// Sleep for the specified number of nanoseconds.
pub fn sleep_ns(ns: u64) {
    let start = now_ns();
    while now_ns().wrapping_sub(start) < ns {
        core::hint::spin_loop();
    }
}

/// Sleep for the specified number of milliseconds.
pub fn sleep_ms(ms: u64) {
    sleep_ns(ms * 1_000_000);
}
