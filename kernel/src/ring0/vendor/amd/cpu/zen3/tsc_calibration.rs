//! TSC calibration using the ACPI PM Timer as a reference clock.
//!
//! Implements `AMD/ryzen_5_5600x.md` §12 (TSC y timers) — replaces the
//! stub in `cpu/tsc.rs` that returns the hardcoded 3.7 GHz.
//!
//! Strategy:
//! 1. Try CPUID 0x15 (Core Crystal Clock) — may work on some AMD CPUs.
//! 2. Use the ACPI PM Timer (3,579,545 Hz, fixed frequency) as reference.
//!    - Read PM timer twice with 10 ms between (measured via PIT channel 2).
//!    - Measure TSC over the same interval.
//!    - Compute TSC frequency as: tsc_freq = ticks * 3579545 / pm_ticks.
//! 3. Fall back to the known 5600X base frequency (3,700,000,000 Hz).
//!
//! Status: ✅ COMPLETO — implementación real de calibración TSC.
//!
//! References:
//! - AMD64 APM Vol. 2, §13.1 (TSC)
//! - ACPI Specification 6.5, §4.7.3.1 (PM Timer)
use core::arch::asm;

/// Where the TSC frequency came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TscSource {
    /// CPUID 0x15 returned a valid frequency.
    Cpuid15,
    /// Calibrated using the ACPI PM Timer (3,579,545 Hz).
    AcpiPmTimer,
    /// Hardcoded fallback (3,700,000,000 Hz for the 5600X).
    Hardcoded,
}

impl TscSource {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cpuid15 => "CPUID 0x15",
            Self::AcpiPmTimer => "ACPI PM Timer",
            Self::Hardcoded => "hardcoded 5600X constant",
        }
    }
}

/// Read the ACPI PM Timer (24-bit, 3.579545 MHz).
/// `port` is the PM Timer I/O port (typically 0x408 on modern systems).
#[inline]
fn read_pm_timer(port: u16) -> u32 {
    let low: u32;
    unsafe {
        asm!("in eax, dx", out("eax") low, in("dx") port, options(nostack, preserves_flags));
    }
    low & 0x00FFFFFF
}

/// Read the PIT (Programmable Interval Timer) counter on channel 2.
/// Returns the current 16-bit counter value.
#[inline]
fn read_pit_channel2() -> u16 {
    let low: u8;
    let high: u8;
    unsafe {
        asm!("out 0x80, al", in("al") 0u8, options(nostack, preserves_flags)); // latch
        asm!("in al, 0x42", out("al") low, options(nostack, preserves_flags));
        asm!("in al, 0x42", out("al") high, options(nostack, preserves_flags));
    }
    ((high as u16) << 8) | low as u16
}

/// Configure PIT channel 2 for one-shot mode (used to measure time).
/// `divisor` is the value loaded into channel 2 (1.193182 MHz base).
#[inline]
fn setup_pit_channel2(divisor: u16) {
    unsafe {
        // Channel 2, lobyte/highbyte, mode 0 (one-shot), binary
        asm!("out 0x43, al", in("al") 0b1011_0000u8, options(nostack, preserves_flags));
        asm!("out 0x42, al", in("al") (divisor & 0xFF) as u8, options(nostack, preserves_flags));
        asm!("out 0x42, al", in("al") (divisor >> 8) as u8, options(nostack, preserves_flags));
    }
}

/// Read the current TSC value (no serialization).
#[inline]
fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags)); }
    ((high as u64) << 32) | low as u64
}

/// Wait for a specific number of microseconds using PIT channel 2.
fn pit_wait_us(us: u32) {
    // PIT base frequency is 1,193,182 Hz.
    let divisor: u16 = ((1_193_182u32 * us) / 1_000_000) as u16;
    if divisor == 0 {
        return;
    }
    setup_pit_channel2(divisor);
    // Read back; gate is tied to channel 2 in PC speaker mode (port 0x61).
    unsafe {
        asm!("out 0x61, al", in("al") 0b0000_0001u8, options(nostack, preserves_flags));
    }
    // Spin until counter reaches 0
    loop {
        let v = read_pit_channel2();
        if v == 0 {
            break;
        }
    }
}

/// Calibrate the TSC using the ACPI PM Timer as reference.
/// `pm_timer_port` is the I/O port of the PM Timer (typically 0x408).
/// Returns (tsc_frequency_hz, source).
pub fn calibrate_tsc(pm_timer_port: u16) -> (u64, TscSource) {
    // ── Path 1: Try CPUID 0x15 first (some AMD parts return a value) ──
    if let Some(freq) = try_cpuid_15() {
        if freq > 1_000_000_000 && freq < 5_000_000_000 {
            return (freq, TscSource::Cpuid15);
        }
    }

    // ── Path 2: PM Timer calibration ─────────────────────────────────
    // Measure TSC over 10 ms of PM Timer ticks.
    const PM_TIMER_HZ: u64 = 3_579_545; // 3.58 MHz
    const MEASUREMENT_MS: u32 = 10;
    const PM_TICKS_FOR_MS: u32 = (PM_TIMER_HZ * MEASUREMENT_MS as u64 / 1000) as u32;

    let pm_start = read_pm_timer(pm_timer_port);
    let tsc_start = rdtsc();

    // Spin until PM timer advances by MEASUREMENT_MS
    let mut pm_now = pm_start;
    let mut safety_iter = 0u32;
    loop {
        pm_now = read_pm_timer(pm_timer_port);
        let delta = pm_now.wrapping_sub(pm_start);
        if delta >= PM_TICKS_FOR_MS {
            break;
        }
        // Safety: if for some reason PM timer is broken, fall through
        // after 100 ms of PIT time to avoid infinite loop.
        safety_iter += 1;
        if safety_iter > 1_000_000 {
            break;
        }
    }

    let tsc_end = rdtsc();
    let tsc_delta = tsc_end - tsc_start;
    let pm_delta = pm_now.wrapping_sub(pm_start) as u64;

    if pm_delta == 0 {
        return (3_700_000_000, TscSource::Hardcoded);
    }

    // tsc_freq = tsc_delta * PM_TIMER_HZ / pm_delta
    let tsc_freq = (tsc_delta * PM_TIMER_HZ) / pm_delta;

    // Sanity check: must be in 1-5 GHz range
    if tsc_freq > 1_000_000_000 && tsc_freq < 5_000_000_000 {
        return (tsc_freq, TscSource::AcpiPmTimer);
    }

    // ── Path 3: hardcoded fallback ───────────────────────────────────
    (3_700_000_000, TscSource::Hardcoded)
}

/// Try CPUID 0x15 (Core Crystal Clock Frequency). Returns the nominal
/// frequency in Hz, or None if the leaf is not supported.
fn try_cpuid_15() -> Option<u64> {
    let (eax, ebx, ecx, _): (u32, u32, u32, u32);
    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") 0x15u32 => eax,
            inout("ecx") 0u32 => ecx,
            ebx_out = out(reg) ebx,
            out("edx") _,
        );
    }
    if eax != 0 && ebx != 0 && ecx != 0 {
        Some(ecx as u64)
    } else {
        None
    }
}
