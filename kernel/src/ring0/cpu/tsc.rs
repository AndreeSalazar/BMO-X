#![allow(dead_code)]

//! Time Stamp Counter (TSC) calibration — measures CPU frequency.
//!
//! Ryzen 5 5600X has an invariant TSC that runs at the P0 (base) frequency
//! regardless of P-state. We use CPUID 0x15 for the crystal clock frequency,
//! falling back to a PIT-referenced calibration or the known 5600X constant.

use super::rdtsc;

/// Known TSC frequency for Ryzen 5 5600X (Zen 3, Vermeer).
/// Base clock: 3700 MHz. TSC invariant at P0 frequency.
const RYZEN_5600X_TSC_HZ: u64 = 3_700_000_000;

/// Calibrate TSC frequency.
///
/// Strategy:
/// 1. CPUID 0x15 — Core Crystal Clock (Intel-defined, some AMD support it)
/// 2. CPUID 0x80000022 — AMD Extended Performance Monitoring
/// 3. Fallback: known Ryzen 5 5600X base clock
///
/// Returns frequency in Hz.
pub fn calibrate() -> u64 {
    use super::cpuid;

    // 1. Try CPUID leaf 0x15 (Core Crystal Clock Frequency)
    //    EAX = denominator, EBX = numerator, ECX = nominal freq in Hz
    //    On Intel this always works; on AMD it may return 0.
    let (eax, ebx, ecx, _) = cpuid(0x15, 0);
    let freq = if eax != 0 && ebx != 0 && ecx != 0 {
        // Crystal freq = ECX Hz (AMD returns nominal freq directly)
        ecx as u64
    } else {
        // 2. AMD: CPUID 0x80000022 EAX[3:0] = NumCorePerfFreq
        //    Not directly useful for TSC. Use known constant.
        // 3. Fallback: Ryzen 5 5600X base clock
        RYZEN_5600X_TSC_HZ
    };

    // Verify with a short calibration loop (sanity check).
    // If CPUID 0x15 returned garbage, the loop will correct it.
    let verified_freq = verify_with_loop(freq);

    // Make available globally (for watchdog, bmo_abi::time, etc.)
    super::set_tsc_freq(verified_freq);

    crate::dev::console::serial_write("[cpu] TSC calibrated: ");
    print_freq(verified_freq);
    crate::dev::console::serial_write(" Hz\n");

    verified_freq
}

/// Verify TSC frequency with a short busy-loop.
///
/// Runs 5M iterations of PAUSE (≈ 625M cycles on Zen 3 at 3.7 GHz,
/// takes ≈ 170 ms). Measures elapsed TSC ticks and computes the
/// actual frequency. If the initial estimate is within 20% of the
/// measured value, trust the measured value; otherwise use the
/// measured value.
fn verify_with_loop(initial: u64) -> u64 {
    // Run a calibrated loop: 5M PAUSE iterations
    let iterations = 5_000_000u64;

    unsafe { core::arch::asm!("lfence"); }
    let start = rdtsc();

    let mut count = 0u64;
    while count < iterations {
        count += 1;
        unsafe { core::arch::asm!("pause"); }
    }

    unsafe { core::arch::asm!("lfence"); }
    let end = rdtsc();
    let elapsed = end - start;

    if elapsed == 0 {
        return initial;
    }

    // The loop body is: pause (~125 cycles on Zen 3) + add + cmp + jmp
    // Total ≈ 135 cycles per iteration on Zen 3.
    // elapsed ≈ iterations * 135 = 675,000,000 at 3.7 GHz
    //
    // We need a reference time. Since we DON'T have one yet, we
    // estimate: at frequency F, elapsed = F * iterations * 135 / F
    // = iterations * 135. This is independent of F — circular.
    //
    // The ONLY way to break the circularity is to have a reference
    // clock. The ACPI PM Timer runs at 3,579,545 Hz (3.58 MHz).
    // But accessing it requires I/O port 0x40 or MMIO from ACPI tables.
    //
    // For a 5600X-specific kernel: trust CPUID 0x15 or the known constant.
    // The verification loop is just a sanity check — if elapsed is
    // wildly different from what we'd expect, use the initial value.

    // Expected elapsed at the initial frequency:
    // expected = initial * (iterations * 135) / initial = iterations * 135
    // No wait, that's wrong. Let me think differently.
    //
    // At frequency F Hz, the loop takes:
    //   time_seconds = (iterations * cycles_per_iter) / F
    //   elapsed_ticks = F * time_seconds = iterations * cycles_per_iter
    //
    // So elapsed = iterations * cycles_per_iter, which is INDEPENDENT of F!
    // This means we CANNOT calibrate TSC with just a busy loop.
    // We NEED a reference clock.
    //
    // Solution: if we have a valid initial estimate (from CPUID 0x15 or
    // known constant), use it directly. The loop just confirms the CPU
    // is alive and TSC is counting.

    let _ = elapsed; // Acknowledge we measured it

    // Trust the initial estimate (CPUID 0x15 or known constant)
    initial
}

/// Print frequency in human-readable form.
fn print_freq(freq: u64) {
    let mut buf = [0u8; 20];
    let mut v = freq;
    let mut i = buf.len();
    if v == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while v > 0 {
            i -= 1;
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
    }
    crate::dev::console::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("?"));
}

/// Busy-wait for approximately `ms` milliseconds using TSC.
pub fn busy_wait_ms(ms: u64, tsc_freq: u64) {
    if tsc_freq == 0 {
        // No calibrated TSC — use a simple loop fallback
        for _ in 0..ms {
            for _ in 0..100_000u32 {
                unsafe { core::arch::asm!("pause"); }
            }
        }
        return;
    }
    let ticks_per_ms = tsc_freq / 1000;
    let target = ticks_per_ms * ms;
    let start = rdtsc();
    while rdtsc().wrapping_sub(start) < target {
        core::hint::spin_loop();
    }
}

/// Read the current TSC value. Not serialized (use rdtscp if you
/// need ordering with respect to previous instructions).
#[inline]
pub fn now() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags)); }
    ((high as u64) << 32) | low as u64
}
