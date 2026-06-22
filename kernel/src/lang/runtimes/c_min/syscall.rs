//! `c_min::syscall` — Thin wrappers sobre `syscall` para BMO ABI.
//!
//! Cada función genera un `syscall` con el número canónico del ABI
//! (ver `bmo_abi::syscalls`).

#![allow(dead_code)]

use crate::bmo_abi::syscalls;

/// `void bmo_exit(int code)`.
pub unsafe extern "C" fn bmo_exit(code: i32) -> ! {
    core::arch::asm!(
        "mov rax, {nr}",
        "mov rdi, {code}",
        "syscall",
        nr = const (syscalls::NR_PROC_EXIT as u64),
        code = in(reg) code as u64,
        options(noreturn)
    );
}

/// `int bmo_fs_open(const char *path, int flags) -> fd`.
pub unsafe extern "C" fn bmo_fs_open(path: *const u8, flags: i32) -> i64 {
    let ret: i64;
    core::arch::asm!(
        "mov rax, {nr}",
        "mov rdi, {path}",
        "mov rsi, {flags}",
        "syscall",
        nr = const (syscalls::NR_FS_OPEN as u64),
        path = in(reg) path as u64,
        flags = in(reg) flags as u64,
        lateout("rax") ret,
    );
    ret
}

/// `int bmo_fs_close(int fd)`.
pub unsafe extern "C" fn bmo_fs_close(fd: i32) -> i32 {
    let ret: i32;
    core::arch::asm!(
        "mov rax, {nr}",
        "mov rdi, {fd}",
        "syscall",
        nr = const (syscalls::NR_FS_CLOSE as u64),
        fd = in(reg) fd as u64,
        lateout("rax") ret,
    );
    ret
}

/// `int bmo_fs_read(int fd, void *buf, int len) -> bytes_read`.
pub unsafe extern "C" fn bmo_fs_read(fd: i32, buf: *mut u8, len: i32) -> i32 {
    let ret: i32;
    core::arch::asm!(
        "mov rax, {nr}",
        "mov rdi, {fd}",
        "mov rsi, {buf}",
        "mov rdx, {len}",
        "syscall",
        nr = const (syscalls::NR_FS_READ as u64),
        fd = in(reg) fd as u64,
        buf = in(reg) buf as u64,
        len = in(reg) len as u64,
        lateout("rax") ret,
    );
    ret
}

/// `int bmo_fs_write(int fd, const void *buf, int len) -> bytes_written`.
pub unsafe extern "C" fn bmo_fs_write(fd: i32, buf: *const u8, len: i32) -> i32 {
    let ret: i32;
    core::arch::asm!(
        "mov rax, {nr}",
        "mov rdi, {fd}",
        "mov rsi, {buf}",
        "mov rdx, {len}",
        "syscall",
        nr = const (syscalls::NR_FS_WRITE as u64),
        fd = in(reg) fd as u64,
        buf = in(reg) buf as u64,
        len = in(reg) len as u64,
        lateout("rax") ret,
    );
    ret
}

/// `void bmo_diag_print(const char *s, int len)`.
pub unsafe extern "C" fn bmo_diag_print(s: *const u8, len: i32) {
    core::arch::asm!(
        "mov rax, {nr}",
        "mov rdi, {s}",
        "mov rsi, {len}",
        "syscall",
        nr = const (syscalls::NR_DEBUG_PRINT as u64),
        s = in(reg) s as u64,
        len = in(reg) len as u64,
    );
}

/// `u64 bmo_time_now_ns() -> nanoseconds since boot`.
pub unsafe extern "C" fn bmo_time_now_ns() -> u64 {
    let ret: u64;
    core::arch::asm!(
        "mov rax, {nr}",
        "syscall",
        nr = const (syscalls::NR_TIME_NOW_NS as u64),
        lateout("rax") ret,
    );
    ret
}
