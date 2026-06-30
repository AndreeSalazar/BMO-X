//! Linux libc function implementations (C ABI)
//!
//! Each function here is the target of an ELF symbol thunk.
//! When an ELF binary calls `write(1, buf, 10)`, the shim layer
//! redirects to `shims::linux::libc::write()`.

use super::syscall;

#[no_mangle]
pub extern "C" fn write(fd: i32, buf: *const u8, count: usize) -> isize {
    syscall::dispatch(syscall::SYS_WRITE, &[fd as u64, buf as u64, count as u64, 0, 0, 0]) as isize
}

#[no_mangle]
pub extern "C" fn read(fd: i32, buf: *mut u8, count: usize) -> isize {
    syscall::dispatch(syscall::SYS_READ, &[fd as u64, buf as u64, count as u64, 0, 0, 0]) as isize
}

#[no_mangle]
pub extern "C" fn open(path: *const u8, flags: i32, mode: u16) -> i32 {
    syscall::dispatch(syscall::SYS_OPEN, &[path as u64, flags as u64, mode as u64, 0, 0, 0]) as i32
}

#[no_mangle]
pub extern "C" fn close(fd: i32) -> i32 {
    syscall::dispatch(syscall::SYS_CLOSE, &[fd as u64, 0, 0, 0, 0, 0]) as i32
}

#[no_mangle]
pub extern "C" fn exit(code: i32) -> ! {
    syscall::dispatch(syscall::SYS_EXIT, &[code as u64, 0, 0, 0, 0, 0]);
    loop { unsafe { core::arch::asm!("hlt"); } }
}

#[no_mangle]
pub extern "C" fn exit_group(code: i32) -> ! {
    syscall::dispatch(syscall::SYS_EXIT_GROUP, &[code as u64, 0, 0, 0, 0, 0]);
    loop { unsafe { core::arch::asm!("hlt"); } }
}

#[no_mangle]
pub extern "C" fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: isize) -> *mut u8 {
    syscall::dispatch(syscall::SYS_MMAP, &[addr as u64, len as u64, prot as u64, flags as u64, fd as u64, off as u64]) as *mut u8
}

#[no_mangle]
pub extern "C" fn munmap(addr: *mut u8, len: usize) -> i32 {
    syscall::dispatch(syscall::SYS_MUNMAP, &[addr as u64, len as u64, 0, 0, 0, 0]) as i32
}

#[no_mangle]
pub extern "C" fn brk(addr: *mut u8) -> *mut u8 {
    syscall::dispatch(syscall::SYS_BRK, &[addr as u64, 0, 0, 0, 0, 0]) as *mut u8
}

#[no_mangle]
pub extern "C" fn sched_yield() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn getpid() -> i32 { 1 }

#[no_mangle]
pub extern "C" fn gettid() -> i32 { 1 }

#[no_mangle]
pub extern "C" fn nanosleep(req: *const u8, rem: *mut u8) -> i32 {
    syscall::dispatch(syscall::SYS_NANOSLEEP, &[req as u64, rem as u64, 0, 0, 0, 0]) as i32
}

#[no_mangle]
pub extern "C" fn clock_gettime(clk_id: u64, tp: *mut u8) -> i32 {
    syscall::dispatch(syscall::SYS_CLOCK_GETTIME, &[clk_id, tp as u64, 0, 0, 0, 0]) as i32
}

#[no_mangle]
pub extern "C" fn uname(buf: *mut u8) -> i32 {
    syscall::dispatch(syscall::SYS_UNAME, &[buf as u64, 0, 0, 0, 0, 0]) as i32
}

#[no_mangle]
pub extern "C" fn getcwd(buf: *mut u8, size: usize) -> *mut u8 {
    let ret = syscall::dispatch(syscall::SYS_GETCWD, &[buf as u64, size as u64, 0, 0, 0, 0]);
    if ret < 0 { core::ptr::null_mut() } else { buf }
}

#[no_mangle]
pub extern "C" fn lseek(fd: i32, offset: isize, whence: i32) -> isize {
    syscall::dispatch(syscall::SYS_LSEEK, &[fd as u64, offset as u64, whence as u64, 0, 0, 0]) as isize
}

#[no_mangle]
pub extern "C" fn ioctl(fd: i32, request: u64, argp: *mut u8) -> i32 {
    syscall::dispatch(syscall::SYS_IOCTL, &[fd as u64, request, argp as u64, 0, 0, 0]) as i32
}

#[no_mangle]
pub extern "C" fn access(path: *const u8, mode: i32) -> i32 {
    syscall::dispatch(syscall::SYS_ACCESS, &[path as u64, mode as u64, 0, 0, 0, 0]) as i32
}

#[no_mangle]
pub extern "C" fn fcntl(fd: i32, cmd: i32, arg: u64) -> i32 {
    syscall::dispatch(syscall::SYS_FCNTL, &[fd as u64, cmd as u64, arg, 0, 0, 0]) as i32
}

#[no_mangle]
pub extern "C" fn fstat(fd: i32, buf: *mut u8) -> i32 {
    syscall::dispatch(syscall::SYS_FSTAT, &[fd as u64, buf as u64, 0, 0, 0, 0]) as i32
}
