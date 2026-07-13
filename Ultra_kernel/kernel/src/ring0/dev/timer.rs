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

/// Global timer state. Defaults to TSC (always available after
/// `tsc::calibrate()` ran). The timer subsystem upgrades to HPET if
/// one is detected.
static mut CURRENT_SOURCE: TimerSource = TimerSource::Tsc;

/// Returns the currently active timer source.
pub fn current_source() -> TimerSource {
    unsafe { CURRENT_SOURCE }
}

/// Initialize the timer subsystem.
pub fn init() {
    crate::ring0::dev::console::serial_write("[timer] initializing\n");

    // Try HPET first
    if super::hpet::is_available() {
        super::hpet::init();
        unsafe { CURRENT_SOURCE = TimerSource::Hpet; }
        crate::ring0::dev::console::serial_write("[timer] using HPET as clock source\n");
    } else {
        crate::ring0::dev::console::serial_write("[timer] HPET not available, using TSC\n");
    }

    // Init timer wheel
    super::timer_wheel::init();
    crate::ring0::dev::console::serial_write("[timer] timer wheel initialized\n");
}

/// Get current timestamp in nanoseconds.
/// Also updates the vDSO page for Ring 3 fast access.
pub fn now_ns() -> u64 {
    crate::ring0::mm::vdso::tick();
    let source = unsafe { CURRENT_SOURCE };
    match source {
        TimerSource::Hpet => super::hpet::now_ns(),
        TimerSource::ApicTimer | TimerSource::Tsc | TimerSource::Pit =>
            super::timestamp::tsc_to_ns(crate::ring0::cpu::rdtsc()),
    }
}

use core::sync::atomic::{AtomicBool, Ordering};

/// Flag used by timer wheel callback to signal sleep completion.
static SLEEP_DONE: AtomicBool = AtomicBool::new(false);

/// Timer wheel callback that sets the sleep-done flag.
fn sleep_callback(_id: u64) {
    SLEEP_DONE.store(true, Ordering::Release);
}

/// Sleep for the specified number of nanoseconds.
/// For delays >= 10ms, uses the timer wheel to avoid burning CPU.
/// For shorter delays, uses spin-wait (timer wheel resolution is 1ms).
pub fn sleep_ns(ns: u64) {
    const WHEEL_THRESHOLD_NS: u64 = 10_000_000; // 10 ms

    if ns >= WHEEL_THRESHOLD_NS {
        // Use timer wheel: register a callback, spin on the flag
        SLEEP_DONE.store(false, Ordering::Release);
        super::timer_wheel::add_timer(ns, sleep_callback);
        while !SLEEP_DONE.load(Ordering::Acquire) {
            core::hint::spin_loop();
        }
    } else {
        // Short delay: spin-wait directly
        let start = now_ns();
        while now_ns().wrapping_sub(start) < ns {
            core::hint::spin_loop();
        }
    }
}

/// Sleep for the specified number of milliseconds.
pub fn sleep_ms(ms: u64) {
    sleep_ns(ms * 1_000_000);
}
