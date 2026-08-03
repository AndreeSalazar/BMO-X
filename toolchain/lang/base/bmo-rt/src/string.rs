//! C standard library string functions for Ring 3 BEF apps.
//!
//! All functions are `#![no_std]` and `extern "C"` so C, COBOL, and
//! Rust programs can call them. No heap, no syscalls, pure computation.

/// Copy `n` bytes from `src` to `dest`. Overlap-safe via direction flag.
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if n == 0 || dest.is_null() || src.is_null() { return dest; }
    let d = dest;
    let s = src;
    if d as usize > s as usize {
        // Copy backwards if dest overlaps src
        for i in (0..n).rev() {
            *d.add(i) = *s.add(i);
        }
    } else {
        for i in 0..n {
            *d.add(i) = *s.add(i);
        }
    }
    dest
}

/// Copy `n` bytes from `src` to `dest`. Handles overlap correctly.
#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    memcpy(dest, src, n)
}

/// Set `n` bytes at `s` to value `c`.
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    if n == 0 || s.is_null() { return s; }
    for i in 0..n {
        *s.add(i) = c as u8;
    }
    s
}

/// Compare `n` bytes of `s1` and `s2`. Returns 0 if equal, <0 or >0 otherwise.
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    if n == 0 { return 0; }
    for i in 0..n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b { return a as i32 - b as i32; }
    }
    0
}

/// Count the length of a null-terminated string.
#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const u8) -> usize {
    if s.is_null() { return 0; }
    let mut n = 0;
    while *s.add(n) != 0u8 {
        n += 1;
    }
    n
}

/// Compare two null-terminated strings.
#[no_mangle]
pub unsafe extern "C" fn strcmp(s1: *const u8, s2: *const u8) -> i32 {
    let mut i = 0;
    loop {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b { return a as i32 - b as i32; }
        if a == 0 { return 0; }
        i += 1;
    }
}

/// Compare up to `n` characters of `s1` and `s2`.
#[no_mangle]
pub unsafe extern "C" fn strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b { return a as i32 - b as i32; }
        if a == 0 { return 0; }
    }
    0
}

/// Copy `src` to `dest`, including the null terminator.
#[no_mangle]
pub unsafe extern "C" fn strcpy(dest: *mut u8, src: *const u8) -> *mut u8 {
    let mut i = 0;
    loop {
        let c = *src.add(i);
        *dest.add(i) = c;
        if c == 0 { break; }
        i += 1;
    }
    dest
}

/// Copy at most `n` characters from `src` to `dest`, padding with \0.
#[no_mangle]
pub unsafe extern "C" fn strncpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        let c = *src.add(i);
        *dest.add(i) = c;
        if c == 0 {
            i += 1;
            while i < n { *dest.add(i) = 0; i += 1; }
            break;
        }
        i += 1;
    }
    dest
}

/// Find the first occurrence of `c` in `s`. Returns pointer or null.
#[no_mangle]
pub unsafe extern "C" fn strchr(s: *const u8, c: i32) -> *const u8 {
    let mut i = 0;
    loop {
        let ch = *s.add(i);
        if ch == c as u8 { return s.add(i); }
        if ch == 0 { break; }
        i += 1;
    }
    core::ptr::null()
}

/// Find the last occurrence of `c` in `s`.
#[no_mangle]
pub unsafe extern "C" fn strrchr(s: *const u8, c: i32) -> *const u8 {
    let len = strlen(s);
    for i in (0..=len).rev() {
        if *s.add(i) == c as u8 { return s.add(i); }
    }
    core::ptr::null()
}

/// Find `needle` in `haystack`. Returns pointer or null.
#[no_mangle]
pub unsafe extern "C" fn strstr(haystack: *const u8, needle: *const u8) -> *const u8 {
    if *needle == 0 { return haystack; }
    let nlen = strlen(needle);
    let hlen = strlen(haystack);
    if nlen > hlen { return core::ptr::null(); }
    for i in 0..=(hlen - nlen) {
        if memcmp(haystack.add(i), needle, nlen) == 0 {
            return haystack.add(i);
        }
    }
    core::ptr::null()
}

/// Duplicate a string (malloc + copy). Returns pointer to copy.
/// Caller must free() the result.
#[no_mangle]
pub unsafe extern "C" fn strdup(s: *const u8) -> *mut u8 {
    let len = strlen(s);
    let p = crate::heap::malloc(len + 1) as *mut u8;
    if p.is_null() { return p; }
    strcpy(p, s);
    p
}

/// Duplicate at most `n` characters.
#[no_mangle]
pub unsafe extern "C" fn strndup(s: *const u8, n: usize) -> *mut u8 {
    let len = strlen(s).min(n);
    let p = crate::heap::malloc(len + 1) as *mut u8;
    if p.is_null() { return p; }
    for i in 0..len { *p.add(i) = *s.add(i); }
    *p.add(len) = 0;
    p
}

/// Append `src` to `dest`. dest must have enough space.
#[no_mangle]
pub unsafe extern "C" fn strcat(dest: *mut u8, src: *const u8) -> *mut u8 {
    let dlen = strlen(dest);
    strcpy(dest.add(dlen), src);
    dest
}

/// Append at most `n` characters.
#[no_mangle]
pub unsafe extern "C" fn strncat(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let dlen = strlen(dest);
    strncpy(dest.add(dlen), src, n);
    dest
}
