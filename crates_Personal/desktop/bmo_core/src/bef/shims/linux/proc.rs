use super::errno;

pub fn sys_exit(a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    crate::cabina::info_u64("linux", "exit", a0);
    loop { unsafe { core::arch::asm!("hlt"); } }
}

pub fn sys_exit_group(a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    crate::cabina::info_u64("linux", "exit_group", a0);
    loop { unsafe { core::arch::asm!("hlt"); } }
}

pub fn sys_getpid(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 { 1 }

pub fn sys_gettid(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    crate::proc::task::current().map(|t| t.tid.0 as i64).unwrap_or(1)
}

pub fn sys_sched_yield(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 { 0 }

pub fn sys_uname(a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let buf = a0 as *mut u8;
    if buf.is_null() { return -errno::EFAULT; }
    let uts = b"Linux\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    unsafe { core::ptr::copy_nonoverlapping(uts.as_ptr(), buf, uts.len().min(65)); }
    0
}

pub fn sys_getcwd(a0: u64, a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let buf = a0 as *mut u8;
    let size = a1 as usize;
    if buf.is_null() || size == 0 { return -errno::EFAULT; }
    let cwd = b"/\x00";
    if cwd.len() > size { return -errno::ERANGE; }
    unsafe { core::ptr::copy_nonoverlapping(cwd.as_ptr(), buf, cwd.len()); }
    cwd.len() as i64
}

pub fn sys_set_robust_list(a0: u64, a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    if let Some(t) = crate::proc::task::current() {
        t.robust_list_head = a0;
        0
    } else { -errno::ESRCH }
}

pub fn sys_set_tid_address(a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    if let Some(t) = crate::proc::task::current() {
        t.tid_address = a0 as *mut i32;
        t.tid.0 as i64
    } else { -errno::ESRCH }
}

pub fn sys_getrandom(a0: u64, a1: u64, a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    let buf = a0 as *mut u8;
    let buflen = a1 as usize;
    let _flags = a2 as u32;
    if buf.is_null() { return -errno::EFAULT; }
    let tsc = crate::cpu::rdtsc();
    let mut state = tsc.wrapping_add(buf as u64);
    for i in 0..buflen {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        unsafe { *buf.add(i) = (state >> 32) as u8; }
    }
    buflen as i64
}
