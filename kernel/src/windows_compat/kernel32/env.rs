//! kernel32.dll — Environment and command line.

#![allow(dead_code)]

static mut COMMAND_LINE: [u8; 256] = [0; 256];

/// GetCommandLineA — get process command line (ASCII).
#[no_mangle]
pub extern "C" fn GetCommandLineA() -> u64 {
    unsafe { COMMAND_LINE.as_ptr() as u64 }
}

/// GetCommandLineW — get process command line (UTF-16).
#[no_mangle]
pub extern "C" fn GetCommandLineW() -> u64 {
    // TODO: UTF-16 version
    unsafe { COMMAND_LINE.as_ptr() as u64 }
}

/// GetEnvironmentStrings — get environment block.
#[no_mangle]
pub extern "C" fn GetEnvironmentStrings() -> u64 {
    // Return empty environment (null-terminated empty string)
    static EMPTY: [u8; 2] = [0, 0];
    EMPTY.as_ptr() as u64
}

/// FreeEnvironmentStrings — free environment block.
#[no_mangle]
pub extern "C" fn FreeEnvironmentStringsA(_env: u64) -> u64 {
    1
}
