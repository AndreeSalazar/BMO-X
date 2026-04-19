//! PIT 8253 — Programmable Interval Timer.
//! Channel 0, ~100 Hz tick for uptime tracking.

use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

const PIT_CH0: u16 = 0x40;
const PIT_CMD: u16 = 0x43;
const PIT_FREQ: u32 = 1193182;
const TARGET_HZ: u32 = 100;

/// Global tick counter — incremented by IRQ0 handler.
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Initialize PIT Channel 0 at ~100 Hz.
pub fn init_pit() {
    let divisor = (PIT_FREQ / TARGET_HZ) as u16;

    // Channel 0, lobyte/hibyte, rate generator
    outb(PIT_CMD, 0x36);
    outb(PIT_CH0, (divisor & 0xFF) as u8);
    outb(PIT_CH0, (divisor >> 8) as u8);
}

/// Called from IRQ0 handler.
pub fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// Get current tick count.
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Get uptime in seconds.
pub fn uptime_secs() -> u64 {
    ticks() / TARGET_HZ as u64
}

#[inline]
fn outb(port: u16, val: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") val, options(nostack, preserves_flags)); }
}
