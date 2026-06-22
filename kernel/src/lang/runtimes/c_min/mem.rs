//! `c_min::mem` — memcpy, memset, memmove, memcmp.

#![allow(dead_code)]

/// Firma: `void *memcpy(void *dst, const void *src, size_t n)`.
pub unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dst.add(i) = *src.add(i);
        i += 1;
    }
    dst
}

/// Firma: `void *memset(void *dst, int c, size_t n)`.
pub unsafe extern "C" fn memset(dst: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dst.add(i) = c as u8;
        i += 1;
    }
    dst
}

/// Firma: `void *memmove(void *dst, const void *src, size_t n)`.
pub unsafe extern "C" fn memmove(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dst as usize) < (src as usize) {
        let mut i = 0;
        while i < n { *dst.add(i) = *src.add(i); i += 1; }
    } else {
        let mut i = n;
        while i > 0 { i -= 1; *dst.add(i) = *src.add(i); }
    }
    dst
}

/// Firma: `int memcmp(const void *a, const void *b, size_t n)`.
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let x = *a.add(i) as i32;
        let y = *b.add(i) as i32;
        if x != y { return x - y; }
        i += 1;
    }
    0
}
