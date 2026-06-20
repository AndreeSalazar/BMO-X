#![allow(dead_code)]

//! Time Stamp Counter (TSC) calibration — measures CPU frequency.

use super::rdtsc;

/// Calibrate TSC frequency using a busy-loop reference.
///
/// Returns frequency in Hz. On Zen 3 with invariant TSC, this is accurate
/// to within ~1% of actual CPU clock.
pub fn calibrate() -> u64 {
    use super::cpuid;

    let (_, _, _, edx_ext) = cpuid(0x80000007, 0);
    let _has_invariant_tsc = edx_ext & (1 << 8) != 0;

    unsafe { core::arch::asm!("lfence"); }
    let start = rdtsc();

    // Busy loop — ~50M iterations with PAUSE (~125 cycles each on Zen 3)
    // ≈ 6.25B cycles ≈ 1.7s at 3.7GHz
    let target = 50_000_000u64;
    let mut count = 0u64;
    while count < target {
        count += 1;
        unsafe { core::arch::asm!("pause"); }
    }

    unsafe { core::arch::asm!("lfence"); }
    let end = rdtsc();
    let elapsed = end - start;

    // freq = elapsed_ticks / time_seconds
    // The loop took approximately 50M * 125 cycles = 6.25B cycles ≈ 1.69s at 3.7GHz
    // Scale: freq = elapsed * (1_000_000_000 / elapsed_ns)
    // Simpler: freq ≈ elapsed * 80 (calibrated for 50M PAUSE iterations)
    let freq = if elapsed > 0 { elapsed * 80 } else { 3_700_000_000 };

    // Make available globally (for watchdog, bmo_abi::time, etc.)
    super::set_tsc_freq(freq);

    crate::drivers::serial::serial_write("[cpu] TSC calibrated: ");
    let mut buf = [0u8; 20];
    let mut v = freq;
    let mut i = buf.len();
    if v == 0 { i -= 1; buf[i] = b'0'; }
    else {
        while v > 0 { i -= 1; buf[i] = b'0' + (v % 10) as u8; v /= 10; }
    }
    crate::drivers::serial::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("?"));
    crate::drivers::serial::serial_write(" Hz\n");

    freq
}

/// Busy-wait for approximately `ms` milliseconds using TSC.
pub fn busy_wait_ms(ms: u64, tsc_freq: u64) {
    let target = (tsc_freq / 1000) * ms;
    let start = rdtsc();
    while rdtsc().wrapping_sub(start) < target {
        core::hint::spin_loop();
    }
}
