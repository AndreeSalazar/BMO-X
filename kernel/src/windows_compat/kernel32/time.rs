//! kernel32.dll — Time functions.
//!
//! Real implementations using BMO's TSC-based timing.

#![allow(dead_code)]

/// SleepEx — sleep with alertable flag.
#[no_mangle]
pub extern "C" fn SleepEx(ms: u32, _alertable: u32) {
    super::thread::Sleep(ms);
}

/// GetTickCount — milliseconds since boot.
///
/// Real implementation: uses BMO TSC (Ryzen 5 5600X @ ~3.7GHz).
#[no_mangle]
pub extern "C" fn GetTickCount() -> u32 {
    (crate::arch::cpu::rdtsc() / 3_700_000) as u32
}

/// GetTickCount64 — 64-bit milliseconds since boot.
#[no_mangle]
pub extern "C" fn GetTickCount64() -> u64 {
    crate::arch::cpu::rdtsc() / 3_700_000
}

/// QueryPerformanceCounter — high-resolution timer.
#[no_mangle]
pub extern "C" fn QueryPerformanceCounter(perf_counter: *mut i64) -> u64 {
    if !perf_counter.is_null() {
        unsafe { *perf_counter = crate::arch::cpu::rdtsc() as i64; }
    }
    1 // TRUE
}

/// QueryPerformanceFrequency — high-resolution timer frequency.
#[no_mangle]
pub extern "C" fn QueryPerformanceFrequency(perf_freq: *mut i64) -> u64 {
    if !perf_freq.is_null() {
        unsafe { *perf_freq = 3_700_000_000; } // 3.7 GHz
    }
    1 // TRUE
}

/// GetSystemTimeAsFileTime — get system time as FILETIME.
///
/// FILETIME is 100-nanosecond intervals since January 1, 1601.
/// We approximate from TSC.
#[no_mangle]
pub extern "C" fn GetSystemTimeAsFileTime(file_time: *mut u64) {
    if !file_time.is_null() {
        // Convert TSC to 100ns intervals since epoch
        let tsc = crate::arch::cpu::rdtsc();
        let intervals = tsc / 37; // 3.7GHz / 100ns = 37
        unsafe { *file_time = intervals; }
    }
}
