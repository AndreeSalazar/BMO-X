//! msvcrt.dll — CRT initialization (_initterm, __security_init_cookie).

#![allow(dead_code)]

/// _initterm — call initializers in the CRT init table.
#[no_mangle]
pub extern "C" fn _initterm(_begin: u64, _end: u64) {
    // TODO: iterate function pointer table and call each
}

/// _initterm_e — call initializers with error checking.
#[no_mangle]
pub extern "C" fn _initterm_e(_begin: u64, _end: u64) -> i32 { 0 }

/// __security_init_cookie — initialize GS cookie.
#[no_mangle]
pub extern "C" fn __security_init_cookie() {
    // TODO: initialize stack buffer overflow cookie
}

/// __GSHandlerCheck — GS buffer overrun handler.
#[no_mangle]
pub extern "C" fn __GSHandlerCheck(
    _exception_record: u64, _frame: u64, _context: u64, _dispatcher: u64,
) -> i32 {
    crate::diag::fault("wcompat::seh", "GS buffer overrun detected");
    0 // EXCEPTION_CONTINUE_SEARCH
}

/// __CxxFrameHandler3 — C++ exception handler.
#[no_mangle]
pub extern "C" fn __CxxFrameHandler3(
    _exception_record: u64, _frame: u64, _context: u64, _dispatcher: u64,
) -> i32 {
    crate::diag::fault("wcompat::seh", "C++ exception not supported yet");
    0
}
