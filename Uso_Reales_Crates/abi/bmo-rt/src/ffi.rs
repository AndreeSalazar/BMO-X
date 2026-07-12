//! C-compatible FFI exports for `malloc`, `free`, `realloc`, `calloc`.
//!
//! These are `extern "C"` functions that delegate to the global heap.
//! The C/COBOL frontends find them via the `BMO.toml` manifest.

use crate::heap;

/// Allocate `size` bytes. Returns null on failure.
#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    heap::malloc(size)
}

/// Free a pointer previously returned by `malloc`, `realloc`, or `calloc`.
#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut u8) {
    heap::free(ptr)
}

/// Allocate zero-initialized memory for `nmemb * size` bytes.
#[no_mangle]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    heap::calloc(nmemb, size)
}

/// Resize an allocation to `new_size` bytes.
#[no_mangle]
pub unsafe extern "C" fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
    heap::realloc(ptr, new_size)
}
