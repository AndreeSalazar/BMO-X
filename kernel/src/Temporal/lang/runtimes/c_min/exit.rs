//! `c_min::exit` — _exit y atexit (no-op).

#![allow(dead_code)]

use super::syscall::bmo_exit;

/// `_exit(int code)`. NUNCA retorna.
pub unsafe extern "C" fn _exit(code: i32) -> ! {
    bmo_exit(code);
}

/// `int atexit(void (*fn)(void))` — no-op en este runtime.
pub unsafe extern "C" fn atexit(_fn: unsafe extern "C" fn()) -> i32 {
    0
}
