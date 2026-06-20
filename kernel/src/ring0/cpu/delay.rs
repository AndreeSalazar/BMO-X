//! Microsecond and millisecond delays using the TSC.
//!
//! AMDGPU and most other hardware have timeouts in the 100 µs to 10 ms
//! range. Spinning with a millisecond-resolution timer is wasteful at
//! best, broken at worst. This module calibrates against the TSC and
//! provides calibrated busy-waits.

#![allow(dead_code)]

use super::tsc;

/// Convert microseconds to TSC ticks using the calibrated TSC frequency.
/// Returns 0 if the TSC has not been calibrated yet.
#[inline]
pub fn us_to_ticks(us: u64) -> u64 {
    let freq = super::tsc_per_sec();
    if freq == 0 {
        // Fallback: assume 3.7 GHz (typical Ryzen 5 5600X).
        return us * 3_700;
    }
    (us * freq) / 1_000_000
}

/// Spin for approximately `us` microseconds.
pub fn udelay(us: u64) {
    let target = us_to_ticks(us);
    let start = tsc::now();
    while tsc::now().wrapping_sub(start) < target {
        core::hint::spin_loop();
    }
}

/// Spin for approximately `ms` milliseconds.
pub fn mdelay(ms: u64) {
    udelay(ms * 1_000);
}

/// Read the TSC and wait until `deadline_us` microseconds from now.
/// Useful for hardware timeouts:
/// ```ignore
/// let deadline = tsc::now() + us_to_ticks(100);
/// while !mmio_read(reg) & DONE_BIT {
///     if tsc::now() > deadline { return Err(KError::Timeout); }
///     core::hint::spin_loop();
/// }
/// ```
pub fn deadline_us_from_now(us: u64) -> u64 {
    tsc::now() + us_to_ticks(us)
}

/// Check if a deadline has been reached.
pub fn deadline_elapsed(deadline: u64) -> bool {
    tsc::now() >= deadline
}
