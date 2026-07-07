use super::errno;

pub fn sys_nanosleep(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 { 0 }

pub fn sys_clock_gettime(a0: u64, a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let _clk_id = a0;
    let tp = a1 as *mut u8;
    if tp.is_null() { return -errno::EFAULT; }
    unsafe { core::ptr::write_bytes(tp, 0, 16); }
    0
}

pub fn sys_gettimeofday(a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let tv = a0 as *mut u8;
    if tv.is_null() { return -errno::EFAULT; }
    unsafe { core::ptr::write_bytes(tv, 0, 16); }
    0
}
