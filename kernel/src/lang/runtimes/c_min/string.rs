//! `c_min::string` — strlen, strcmp, strcpy.

#![allow(dead_code)]

/// `size_t strlen(const char *s)`.
pub unsafe extern "C" fn strlen(s: *const u8) -> usize {
    let mut n = 0;
    while *s.add(n) != 0 { n += 1; }
    n
}

/// `int strcmp(const char *a, const char *b)`.
pub unsafe extern "C" fn strcmp(a: *const u8, b: *const u8) -> i32 {
    let mut i = 0;
    loop {
        let x = *a.add(i);
        let y = *b.add(i);
        if x != y { return (x as i32) - (y as i32); }
        if x == 0 { return 0; }
        i += 1;
    }
}

/// `char *strcpy(char *dst, const char *src)`.
pub unsafe extern "C" fn strcpy(dst: *mut u8, src: *const u8) -> *mut u8 {
    let mut i = 0;
    loop {
        let c = *src.add(i);
        *dst.add(i) = c;
        if c == 0 { break; }
        i += 1;
    }
    dst
}
