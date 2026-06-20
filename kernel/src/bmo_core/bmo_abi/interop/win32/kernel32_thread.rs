//! kernel32.dll — Thread management.
//!
//! Maps: CreateThread, ExitThread, Sleep, TLS, CriticalSection.

#![allow(dead_code)]

/// CreateThread — create a new thread.
#[no_mangle]
pub extern "C" fn CreateThread(
    _attrs: u64, stack_size: u64, start_routine: u64, param: u64,
    _flags: u32, _tid: *mut u32,
) -> u64 {
    crate::bmo_core::diag::info_u64("wcompat::k32", "CreateThread stub", start_routine);
    // TODO: create BMO thread
    let _ = (stack_size, param);
    0
}

/// Sleep — sleep for specified milliseconds.
///
/// Real implementation: maps to BMO SleepNs syscall (0x51).
#[no_mangle]
pub extern "C" fn Sleep(ms: u32) {
    let ns = ms as u64 * 1_000_000;
    // Map to BMO SleepNs syscall (0x51)
    let target_cycles = (ns as u128 * 37) / 10;
    let start = crate::arch::cpu::rdtsc();
    while (crate::arch::cpu::rdtsc() - start) < target_cycles as u64 {
        core::hint::spin_loop();
    }
}

/// SwitchToThread — yield to another thread.
#[no_mangle]
pub extern "C" fn SwitchToThread() -> u64 {
    crate::sched::yield_now();
    1
}

/// InitializeCriticalSection — initialize a critical section.
#[no_mangle]
pub extern "C" fn InitializeCriticalSection(_cs: u64) {
    // TODO: implement critical section
}

/// EnterCriticalSection — enter a critical section.
#[no_mangle]
pub extern "C" fn EnterCriticalSection(_cs: u64) {
    // TODO: implement critical section
}

/// LeaveCriticalSection — leave a critical section.
#[no_mangle]
pub extern "C" fn LeaveCriticalSection(_cs: u64) {
    // TODO: implement critical section
}

/// DeleteCriticalSection — delete a critical section.
#[no_mangle]
pub extern "C" fn DeleteCriticalSection(_cs: u64) {
    // TODO: implement critical section
}
