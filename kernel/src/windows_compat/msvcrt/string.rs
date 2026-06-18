//! msvcrt.dll — C string functions.

#![allow(dead_code)]

/// strlen — get length of null-terminated string.
#[no_mangle]
pub extern "C" fn strlen(s: u64) -> u64 {
    if s == 0 { return 0; }
    unsafe {
        let mut len = 0;
        let mut p = s as *const u8;
        while *p != 0 { len += 1; p = p.add(1); }
        len
    }
}

/// strcpy — copy string.
#[no_mangle]
pub extern "C" fn strcpy(dst: u64, src: u64) -> u64 {
    if dst == 0 || src == 0 { return 0; }
    unsafe {
        let mut d = dst as *mut u8;
        let mut s = src as *const u8;
        loop {
            *d = *s;
            if *s == 0 { break; }
            d = d.add(1);
            s = s.add(1);
        }
    }
    dst
}

/// strcmp — compare strings.
#[no_mangle]
pub extern "C" fn strcmp(a: u64, b: u64) -> i32 {
    if a == 0 || b == 0 { return 0; }
    unsafe {
        let mut p1 = a as *const u8;
        let mut p2 = b as *const u8;
        loop {
            let c1 = *p1;
            let c2 = *p2;
            if c1 != c2 { return c1 as i32 - c2 as i32; }
            if c1 == 0 { return 0; }
            p1 = p1.add(1);
            p2 = p2.add(1);
        }
    }
}

/// memcpy — copy memory.
#[no_mangle]
pub extern "C" fn memcpy(dst: u64, src: u64, size: u64) -> u64 {
    if dst == 0 || src == 0 { return dst; }
    unsafe {
        core::ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, size as usize);
    }
    dst
}

/// memset — set memory.
#[no_mangle]
pub extern "C" fn memset(dst: u64, val: i32, size: u64) -> u64 {
    if dst == 0 { return 0; }
    unsafe {
        core::ptr::write_bytes(dst as *mut u8, val as u8, size as usize);
    }
    dst
}

/// memcmp — compare memory.
#[no_mangle]
pub extern "C" fn memcmp(a: u64, b: u64, size: u64) -> i32 {
    if a == 0 || b == 0 { return 0; }
    unsafe {
        let p1 = a as *const u8;
        let p2 = b as *const u8;
        for i in 0..size as usize {
            let c1 = *p1.add(i);
            let c2 = *p2.add(i);
            if c1 != c2 { return c1 as i32 - c2 as i32; }
        }
    }
    0
}
