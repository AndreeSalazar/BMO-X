//! kernel32.dll — Process management.
//!
//! Maps: ExitProcess, GetCurrentProcess/Id, CreateProcess, WaitForSingleObject.

#![allow(dead_code)]

/// ExitProcess — terminates the current process.
#[no_mangle]
pub extern "C" fn ExitProcess(exit_code: u32) -> ! {
    crate::bmo_core::diag::info_u64("wcompat::k32", "ExitProcess", exit_code as u64);
    crate::sched::process::kill_current_process(0, exit_code as u64, 0);
}

/// GetCurrentProcess — returns pseudo-handle for current process.
#[no_mangle]
pub extern "C" fn GetCurrentProcess() -> u64 {
    !0u64 // -1 = pseudo-handle for current process
}

/// GetCurrentProcessId — returns PID of current process.
#[no_mangle]
pub extern "C" fn GetCurrentProcessId() -> u32 {
    1 // TODO: real PID
}

/// GetCurrentThread — returns pseudo-handle for current thread.
#[no_mangle]
pub extern "C" fn GetCurrentThread() -> u64 {
    !1u64 // -2 = pseudo-handle for current thread (actually !1)
}

/// GetCurrentThreadId — returns TID of current thread.
#[no_mangle]
pub extern "C" fn GetCurrentThreadId() -> u32 {
    // TODO: real TID
    1
}
