use super::errno;

pub fn sys_write(a0: u64, a1: u64, a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let fd = a0 as i32;
    let buf = a1 as *const u8;
    let count = a2 as usize;
    if fd != 1 && fd != 2 { return -errno::EBADF; }
    if buf.is_null() || count == 0 { return 0; }
    let slice = unsafe { core::slice::from_raw_parts(buf, count) };
    if let Ok(s) = core::str::from_utf8(slice) {
        crate::dev::console::serial_write(s);
    }
    count as i64
}

pub fn sys_read(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    -errno::ENOSYS
}

pub fn sys_open(a0: u64, a1: u64, a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let _path = a0 as *const u8;
    let _flags = a1 as i32;
    let _mode = a2 as u16;
    crate::cabina::info("linux", "open stub");
    -errno::ENOENT
}

pub fn sys_close(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 { 0 }

pub fn sys_lseek(a0: u64, a1: u64, a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let _fd = a0 as i32;
    let _offset = a1 as isize;
    let _whence = a2 as i32;
    0
}

pub fn sys_fstat(a0: u64, a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let _fd = a0 as i32;
    let buf = a1 as *mut u8;
    if buf.is_null() { return -errno::EFAULT; }
    unsafe { core::ptr::write_bytes(buf, 0, 144); }
    0
}

pub fn sys_access(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 { 0 }

pub fn sys_fcntl(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 { 0 }

pub fn sys_ioctl(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 { 0 }
