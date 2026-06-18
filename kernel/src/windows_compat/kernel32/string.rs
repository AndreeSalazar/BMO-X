//! kernel32.dll — String operations (lstrlen, lstrcpy, MultiByteToWideChar).

#![allow(dead_code)]

/// lstrlenA — get length of ASCII string.
#[no_mangle]
pub extern "C" fn lstrlenA(s: u64) -> i32 {
    if s == 0 { return 0; }
    unsafe {
        let mut len = 0;
        let mut p = s as *const u8;
        while *p != 0 { len += 1; p = p.add(1); }
        len
    }
}

/// lstrcpyA — copy ASCII string.
#[no_mangle]
pub extern "C" fn lstrcpyA(dst: u64, src: u64) -> u64 {
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

/// MultiByteToWideChar — convert UTF-8/ASCII to UTF-16.
#[no_mangle]
pub extern "C" fn MultiByteToWideChar(
    _cp: u32, _flags: u32, mb: u64, mb_len: i32,
    wc: u64, wc_len: i32,
) -> i32 {
    if mb == 0 || wc == 0 { return 0; }
    // Simple ASCII → UTF-16 conversion
    let src_len = if mb_len < 0 {
        lstrlenA(mb) as usize
    } else {
        mb_len as usize
    };
    let out_len = src_len.min(wc_len as usize);
    unsafe {
        let src = mb as *const u8;
        let dst = wc as *mut u16;
        for i in 0..out_len {
            *dst.add(i) = *src.add(i) as u16;
        }
    }
    out_len as i32
}

/// WideCharToMultiByte — convert UTF-16 to UTF-8/ASCII.
#[no_mangle]
pub extern "C" fn WideCharToMultiByte(
    _cp: u32, _flags: u32, wc: u64, wc_len: i32,
    mb: u64, mb_len: i32,
    _defchar: u64, _used: *mut u32,
) -> i32 {
    if wc == 0 || mb == 0 { return 0; }
    let src_len = if wc_len < 0 {
        // Wide string is null-terminated
        unsafe {
            let mut len = 0;
            let mut p = wc as *const u16;
            while *p != 0 { len += 1; p = p.add(1); }
            len
        }
    } else {
        wc_len as usize
    };
    let out_len = src_len.min(mb_len as usize);
    unsafe {
        let src = wc as *const u16;
        let dst = mb as *mut u8;
        for i in 0..out_len {
            *dst.add(i) = *src.add(i) as u8;
        }
    }
    out_len as i32
}
