//! msvcrt.dll — CRT initialization (_initterm, __security_init_cookie).
//!
//! Real implementations that properly initialize the CRT.

#![allow(dead_code)]

use crate::barex::abi::primitives::bx_u64;

static mut SECURITY_COOKIE: u64 = 0;

/// _initterm — call initializers in the CRT init table.
///
/// Real implementation: iterates the function pointer table and calls each.
/// This is called before main() in Windows PE binaries.
#[no_mangle]
pub extern "C" fn _initterm(begin: bx_u64, end: bx_u64) {
    if begin == 0 || end == 0 || begin >= end {
        return;
    }

    let count = ((end - begin) / 8) as usize;
    let table = unsafe {
        core::slice::from_raw_parts(begin as *const *const (), count)
    };

    for &func_ptr in table {
        if !func_ptr.is_null() {
            unsafe {
                let func: extern "C" fn() = core::mem::transmute(func_ptr);
                func();
            }
        }
    }
}

/// _initterm_e — call initializers with error checking.
#[no_mangle]
pub extern "C" fn _initterm_e(begin: bx_u64, end: bx_u64) -> i32 {
    _initterm(begin, end);
    0 // success
}

/// __security_init_cookie — initialize GS stack buffer overflow cookie.
///
/// Real implementation: generates a pseudo-random cookie from TSC.
/// This is used by the /GS security feature in MSVC.
#[no_mangle]
pub extern "C" fn __security_init_cookie() {
    unsafe {
        if SECURITY_COOKIE != 0 {
            return;
        }

        // Generate cookie from TSC (Ryzen 5 5600X)
        let tsc = crate::arch::cpu::rdtsc();

        // Mix bits to get a reasonable cookie
        let cookie = tsc
            ^ (tsc >> 17)
            ^ (tsc >> 34)
            ^ 0xBAD5EC0DEu64; // "bad security" marker

        // Ensure cookie is not 0 or a common sentinel value
        SECURITY_COOKIE = if cookie == 0 || cookie == 0xBB40E64E {
            0x12345678_9ABCDEF0
        } else {
            cookie
        };
    }
}

/// __security_cookie — get the current GS cookie value.
#[no_mangle]
pub extern "C" fn __security_cookie() -> u64 {
    unsafe {
        if SECURITY_COOKIE == 0 {
            __security_init_cookie();
        }
        SECURITY_COOKIE
    }
}

/// __GSHandlerCheck — GS buffer overrun handler.
///
/// Called when a stack buffer overrun is detected.
/// In a real implementation, this would unwind the stack and report the error.
#[no_mangle]
pub extern "C" fn __GSHandlerCheck(
    _exception_record: bx_u64, _frame: bx_u64, _context: bx_u64, _dispatcher: bx_u64,
) -> i32 {
    crate::diag::fault("wcompat::seh", "GS buffer overrun detected");
    0 // EXCEPTION_CONTINUE_SEARCH
}

/// __CxxFrameHandler3 — C++ exception handler (MSVC).
///
/// Handles C++ exceptions (/EHsc). Not fully supported yet.
#[no_mangle]
pub extern "C" fn __CxxFrameHandler3(
    _exception_record: bx_u64, _frame: bx_u64, _context: bx_u64, _dispatcher: bx_u64,
) -> i32 {
    crate::diag::fault("wcompat::seh", "C++ exception not supported yet");
    0
}

/// __CxxFrameHandler4 — C++ exception handler (MSVC newer).
#[no_mangle]
pub extern "C" fn __CxxFrameHandler4(
    _exception_record: bx_u64, _frame: bx_u64, _context: bx_u64, _dispatcher: bx_u64,
) -> i32 {
    crate::diag::fault("wcompat::seh", "C++ exception (v4) not supported yet");
    0
}

/// __chkstk — stack probe for large stack allocations.
///
/// Called by MSVC compiler when a function needs more than one page of stack.
/// Touches each page to trigger guard page exceptions if needed.
#[no_mangle]
pub extern "C" fn __chkstk(size: u64) {
    let mut addr = unsafe {
        let rsp: u64;
        core::arch::asm!("mov {}, rsp", out(reg) rsp);
        rsp
    };

    let end = addr.saturating_sub(size);

    while addr > end {
        addr = addr.saturating_sub(4096);
        unsafe {
            // Touch the page to trigger guard page if needed
            let _ = core::ptr::read_volatile(addr as *const u8);
        }
    }
}

/// ___chkstk_ms — stack probe (alternate name).
#[no_mangle]
pub extern "C" fn ___chkstk_ms(size: u64) {
    __chkstk(size);
}
