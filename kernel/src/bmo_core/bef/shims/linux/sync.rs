use super::errno;

const FUTEX_WAIT: i32 = 0;
const FUTEX_WAKE: i32 = 1;
const FUTEX_REQUEUE: i32 = 3;
const FUTEX_CMP_REQUEUE: i32 = 4;
const FUTEX_PRIVATE_FLAG: i32 = 128;

pub fn sys_futex(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let uaddr = a0 as *mut u32;
    let op = a1 as i32;
    let val = a2 as u32;
    let _timeout = a3 as *const u8;
    let uaddr2 = a4 as *mut u32;
    let val3 = a5 as u32;

    let actual_op = op & !FUTEX_PRIVATE_FLAG;
    match actual_op {
        FUTEX_WAIT => futex_wait(uaddr, val),
        FUTEX_WAKE => futex_wake(uaddr, val),
        FUTEX_REQUEUE | FUTEX_CMP_REQUEUE => futex_requeue(uaddr, val, uaddr2, val3),
        _ => -errno::ENOSYS,
    }
}

fn futex_wait(uaddr: *mut u32, val: u32) -> i64 {
    if uaddr.is_null() { return -errno::EFAULT; }
    let current_val = unsafe { core::ptr::read_volatile(uaddr) };
    if current_val != val { return -errno::EAGAIN; }
    crate::proc::task::block_on(uaddr as u64);
    0
}

fn futex_wake(uaddr: *mut u32, val: u32) -> i64 {
    if uaddr.is_null() { return -errno::EFAULT; }
    crate::proc::task::wake_on(uaddr as u64, val as usize) as i64
}

fn futex_requeue(uaddr: *mut u32, val: u32, uaddr2: *mut u32, val3: u32) -> i64 {
    if uaddr.is_null() || uaddr2.is_null() { return -errno::EFAULT; }
    let current_val = unsafe { core::ptr::read_volatile(uaddr) };
    if current_val != val3 { return -errno::EAGAIN; }
    crate::proc::task::wake_on(uaddr as u64, val as usize) as i64
}
