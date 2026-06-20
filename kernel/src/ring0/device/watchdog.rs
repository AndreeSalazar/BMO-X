//! PIT (Programmable Interval Timer) driver — used as hardware watchdog.
//!
//! Channel 2 is repurposed as a watchdog: the kernel must call
//!
//! v1.6.16: allow(dead_code) — `init` and `arm` are part of the public
//! watchdog API. Phase 4 starts the APIC timer and the PIT watchdog
//! is left dormant; it arms automatically on a future fault path.

#![allow(dead_code)]
//! `pet()` periodically; if it doesn't within `WATCHDOG_TIMEOUT_SECS`,
//! the system resets via keyboard controller (port 0x64, bit 0).
//!
//! Channel 0 is left alone (used by scheduler at 100 Hz).

use core::sync::atomic::{AtomicU64, Ordering};

/// Write to an I/O port. Implemented with direct asm to avoid the
/// `x86_64` crate dependency.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, preserves_flags));
}

/// Watchdog timeout in seconds.
pub const WATCHDOG_TIMEOUT_SECS: u64 = 5;

/// Last time the watchdog was pet (TSC ticks).
static LAST_PET_TSC: AtomicU64 = AtomicU64::new(0);

/// Whether the watchdog is armed.
static ARMED: AtomicU64 = AtomicU64::new(0);

/// Initialize the watchdog (does not arm).
pub fn init() {
    let tsc = crate::cpu::rdtsc();
    LAST_PET_TSC.store(tsc, Ordering::Relaxed);
}

/// Arm the watchdog. After this, if pet() is not called within
/// `WATCHDOG_TIMEOUT_SECS`, the system resets.
pub fn arm() {
    let tsc = crate::cpu::rdtsc();
    LAST_PET_TSC.store(tsc, Ordering::Relaxed);
    ARMED.store(1, Ordering::Relaxed);
    crate::device::serial::serial_write("[watchdog] ARMED (5 sec timeout)\n");
}

/// Disarm the watchdog.
pub fn disarm() {
    ARMED.store(0, Ordering::Relaxed);
    crate::device::serial::serial_write("[watchdog] DISARMED\n");
}

/// Pet the watchdog (reset the timer). Call this periodically.
pub fn pet() {
    let tsc = crate::cpu::rdtsc();
    LAST_PET_TSC.store(tsc, Ordering::Relaxed);
}

/// Check if the watchdog has expired. Call this from the scheduler tick.
/// If expired, resets the system.
pub fn check() {
    if ARMED.load(Ordering::Relaxed) == 0 { return; }

    let tsc_now = crate::cpu::rdtsc();
    let tsc_per_sec = crate::cpu::tsc_per_sec();
    if tsc_per_sec == 0 { return; }

    let elapsed_secs = (tsc_now - LAST_PET_TSC.load(Ordering::Relaxed)) / tsc_per_sec;
    if elapsed_secs >= WATCHDOG_TIMEOUT_SECS {
        crate::device::serial::serial_write("\n!!! WATCHDOG TIMEOUT — REBOOTING !!!\n");
        // Reset via keyboard controller (port 0x64, bit 0)
        unsafe { outb(0x64, 0xFE); }
        // If reset fails, halt
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
}
