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
    crate::diag::info_u64("wcompat::k32", "CreateThread stub", start_routine);
    // TODO: create BMO thread
    let _ = (stack_size, param);
    0
}

/// Sleep — sleep for specified milliseconds.
#[no_mangle]
pub extern "C" fn Sleep(ms: u32) {
    let ns = ms as u64 * 1_000_000;
    crate::diag::info_u64("wcompat::k32", "Sleep", ms as u64);
    // TODO: map to BMO sleep syscall
    let _ = ns;
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
