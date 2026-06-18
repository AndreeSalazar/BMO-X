//! msvcrt.dll — C stdlib (exit, atexit, atoi, getenv).

#![allow(dead_code)]

/// exit — terminate process.
#[no_mangle]
pub extern "C" fn exit(code: i32) -> ! {
    crate::sched::process::kill_current_process(0, code as u64, 0);
}

/// _exit — terminate process immediately.
#[no_mangle]
pub extern "C" fn _exit(code: i32) -> ! {
    crate::sched::process::kill_current_process(0, code as u64, 0);
}

/// atexit — register exit handler.
#[no_mangle]
pub extern "C" fn atexit(_func: u64) -> i32 { 0 }

/// atoi — convert string to integer.
#[no_mangle]
pub extern "C" fn atoi(s: u64) -> i32 {
    if s == 0 { return 0; }
    unsafe {
        let mut p = s as *const u8;
        let mut result: i32 = 0;
        let mut neg = false;
        while *p == b' ' || *p == b'\t' { p = p.add(1); }
        if *p == b'-' { neg = true; p = p.add(1); }
        else if *p == b'+' { p = p.add(1); }
        while *p >= b'0' && *p <= b'9' {
            result = result * 10 + (*p - b'0') as i32;
            p = p.add(1);
        }
        if neg { -result } else { result }
    }
}

/// getenv — get environment variable.
#[no_mangle]
pub extern "C" fn getenv(_name: u64) -> u64 { 0 }

/// _errno — get errno address.
#[no_mangle]
pub extern "C" fn _errno() -> u64 {
    static mut ERRNO: i32 = 0;
    unsafe { &mut ERRNO as *mut i32 as u64 }
}
