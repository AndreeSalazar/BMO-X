//! kernel32.dll — Time functions.

#![allow(dead_code)]

/// Sleep — sleep for specified milliseconds.
#[no_mangle]
pub extern "C" fn SleepEx(ms: u32, _alertable: u32) {
    crate::kernel32::thread::Sleep(ms);
}

/// GetTickCount — milliseconds since boot.
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
pub extern "C" fn QueryPerformanceCounter(perf_freq: *mut i64) -> u64 {
    unsafe {
        *perf_freq = 3_700_000; // ~3.7 GHz TSC
    }
    crate::arch::cpu::rdtsc() as i64 as u64
}

/// QueryPerformanceFrequency — high-resolution timer frequency.
#[no_mangle]
pub extern "C" fn QueryPerformanceFrequency(perf_freq: *mut i64) -> u64 {
    unsafe {
        *perf_freq = 3_700_000;
    }
    1
}
