//! Timestamp Abstraction (Ring 0 HAL).
//!
//! Provides high-resolution timestamps by unifying TSC, HPET, and
//! APIC timer into a single API with nanosecond precision.
//!
//! The timestamp layer is used by:
//!   - Timer wheel (timeout management)
//!   - Sleep functions
//!   - Performance measurement
//!   - Process scheduling (time slices)

/// TSC frequency in Hz (set during CPU init).
static mut TSC_FREQ: u64 = 0;

/// HPET period in femtoseconds (set during HPET init).
static mut HPET_PERIOD_FS: u64 = 0;

/// Initialize timestamp calibration.
pub fn init(tsc_freq: u64, hpet_period_fs: u64) {
    unsafe {
        TSC_FREQ = tsc_freq;
        HPET_PERIOD_FS = hpet_period_fs;
    }
    crate::dev::console::serial_write("[timestamp] TSC=");
    crate::dev::console::serial_write_u64(tsc_freq / 1_000_000, 10);
    crate::dev::console::serial_write(" MHz, HPET period=");
    crate::dev::console::serial_write_u64(hpet_period_fs, 10);
    crate::dev::console::serial_write(" fs\n");
}

/// Convert TSC ticks to nanoseconds.
pub fn tsc_to_ns(ticks: u64) -> u64 {
    let freq = unsafe { TSC_FREQ };
    if freq == 0 { return ticks; }
    ticks * 1_000_000_000 / freq
}

/// Convert nanoseconds to TSC ticks.
pub fn ns_to_tsc(ns: u64) -> u64 {
    let freq = unsafe { TSC_FREQ };
    if freq == 0 { return ns; }
    ns * freq / 1_000_000_000
}

/// Convert HPET ticks to nanoseconds.
pub fn hpet_to_ns(hpet_ticks: u64) -> u64 {
    let period = unsafe { HPET_PERIOD_FS };
    if period == 0 { return hpet_ticks; }
    hpet_ticks * period / 1_000_000 // femtoseconds → nanoseconds
}

/// Get current timestamp in nanoseconds using best available source.
pub fn now_ns() -> u64 {
    if super::hpet::is_available() {
        super::hpet::now_ns()
    } else {
        tsc_to_ns(crate::cpu::rdtsc())
    }
}

/// Calibrate TSC vs HPET for higher accuracy.
/// Runs a spin-wait and compares TSC ticks to HPET ticks.
pub fn calibrate_tsc_hpet() -> u64 {
    if !super::hpet::is_available() {
        return unsafe { TSC_FREQ };
    }

    let tsc_start = crate::cpu::rdtsc();
    let hpet_start = super::hpet::counter();

    // Wait ~10ms
    let target_ns = 10_000_000; // 10ms
    let hpet_start_ns = super::hpet::now_ns();
    loop {
        let elapsed = super::hpet::now_ns() - hpet_start_ns;
        if elapsed >= target_ns { break; }
    }

    let tsc_end = crate::cpu::rdtsc();
    let hpet_end = super::hpet::counter();

    let tsc_delta = tsc_end.wrapping_sub(tsc_start);
    let hpet_ns = super::hpet::now_ns(); // use now_ns for calibrated value

    if hpet_ns > 0 {
        let calibrated = tsc_delta * 1_000_000_000 / hpet_ns;
        unsafe { TSC_FREQ = calibrated; }
        calibrated
    } else {
        unsafe { TSC_FREQ }
    }
}
