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

    // Trust the initial estimate (CPUID 0x15 or known constant).
    // v1.8.7: eliminada `verify_with_loop` (anterior intento de calibrar
    // TSC con un busy-loop). El comentario en su interior explicaba por
    // qué NO puede calibrarse sin un reloj de referencia (ACPI PM Timer):
    //   elapsed_ticks = F * (iterations * cycles_per_iter) / F
    //                 = iterations * cycles_per_iter
    // → independiente de F, no se puede romper la circularidad sin un
    //   reloj externo. Se confía en CPUID 0x15 o el constante conocido
    //   para el 5600X. Cuando se implemente ACPI real, sustituir por
    //   calibración contra el PM Timer (3,579,545 Hz).

    // Make available globally (for watchdog, bmo_abi::time, etc.)
    super::set_tsc_freq(freq);

    crate::dev::console::serial_write("[cpu] TSC calibrated: ");
    print_freq(freq);
    crate::dev::console::serial_write(" Hz\n");

    // v1.8.8: if the global fastos_cpu::tsc_freq_hz is 0 (init not yet run),
    // delegate to the real calibration. Otherwise use the global.
    let final_freq = if crate::amd_cpu::zen3::tsc_freq_hz() != 0 {
        crate::amd_cpu::zen3::tsc_freq_hz()
    } else {
        freq
    };

    final_freq
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
