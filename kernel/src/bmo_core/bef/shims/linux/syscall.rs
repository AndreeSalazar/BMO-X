// Linux x86_64 syscall numbers (subset for hello-world support)
pub const SYS_READ: usize = 0;
pub const SYS_WRITE: usize = 1;
pub const SYS_OPEN: usize = 2;
pub const SYS_CLOSE: usize = 3;
pub const SYS_STAT: usize = 4;
pub const SYS_FSTAT: usize = 5;
pub const SYS_LSEEK: usize = 8;
pub const SYS_MMAP: usize = 9;
pub const SYS_MPROTECT: usize = 10;
pub const SYS_MUNMAP: usize = 11;
pub const SYS_BRK: usize = 12;
pub const SYS_RT_SIGACTION: usize = 13;
pub const SYS_RT_SIGPROCMASK: usize = 14;
pub const SYS_IOCTL: usize = 16;
pub const SYS_READV: usize = 19;
pub const SYS_WRITEV: usize = 20;
pub const SYS_ACCESS: usize = 21;
pub const SYS_PIPE: usize = 22;
pub const SYS_GETDENTS64: usize = 78;
pub const SYS_EXIT: usize = 60;
pub const SYS_EXIT_GROUP: usize = 231;
pub const SYS_GETPID: usize = 39;
pub const SYS_GETTID: usize = 186;
pub const SYS_NANOSLEEP: usize = 35;
pub const SYS_CLOCK_GETTIME: usize = 228;
pub const SYS_UNAME: usize = 63;
pub const SYS_GETCWD: usize = 79;
pub const SYS_CHDIR: usize = 80;
pub const SYS_MKDIR: usize = 83;
pub const SYS_RMDIR: usize = 84;
pub const SYS_UNLINK: usize = 87;
pub const SYS_LINK: usize = 86;
pub const SYS_SYMLINK: usize = 88;
pub const SYS_READLINK: usize = 89;
pub const SYS_SENDFILE: usize = 40;
pub const SYS_DUP: usize = 32;
pub const SYS_DUP2: usize = 33;
pub const SYS_FCNTL: usize = 72;
pub const SYS_TRUNCATE: usize = 76;
pub const SYS_FTRUNCATE: usize = 77;
pub const SYS_GETDENTS: usize = 78;
pub const SYS_SCHED_YIELD: usize = 24;
pub const SYS_GETTIMEOFDAY: usize = 96;

pub fn dispatch(nr: usize, args: &[u64; 6]) -> i64 {
    match nr {
        SYS_WRITE => handle_write(args[0] as i32, args[1] as *const u8, args[2] as usize),
        SYS_EXIT => handle_exit(args[0] as i32),
        SYS_EXIT_GROUP => handle_exit_group(args[0] as i32),
        SYS_BRK => handle_brk(args[0] as *mut u8),
        SYS_MMAP => handle_mmap(args[0] as *mut u8, args[1] as usize, args[2] as i32, args[3] as i32, args[4] as i32, args[5] as isize),
        SYS_MUNMAP => handle_munmap(args[0] as *mut u8, args[1] as usize),
        SYS_OPEN => handle_open(args[0] as *const u8, args[1] as i32, args[2] as u16),
        SYS_READ => handle_read(args[0] as i32, args[1] as *mut u8, args[2] as usize),
        SYS_CLOSE => handle_close(args[0] as i32),
        SYS_GETPID => handle_getpid(),
        SYS_GETTID => handle_gettid(),
        SYS_NANOSLEEP => handle_nanosleep(args[0] as *const u8, args[1] as *mut u8),
        SYS_CLOCK_GETTIME => handle_clock_gettime(args[0] as u64, args[1] as *mut u8),
        SYS_UNAME => handle_uname(args[0] as *mut u8),
        SYS_GETCWD => handle_getcwd(args[0] as *mut u8, args[1] as usize),
        SYS_LSEEK => handle_lseek(args[0] as i32, args[1] as isize, args[2] as i32),
        SYS_IOCTL => handle_ioctl(args[0] as i32, args[1] as u64, args[2] as *mut u8),
        SYS_ACCESS => handle_access(args[0] as *const u8, args[1] as i32),
        SYS_FCNTL => handle_fcntl(args[0] as i32, args[1] as i32, args[2] as u64),
        SYS_SCHED_YIELD => 0,
        SYS_FSTAT => handle_fstat(args[0] as i32, args[1] as *mut u8),
        SYS_RT_SIGACTION => 0, // ignore
        SYS_RT_SIGPROCMASK => 0,
        _ => {
            crate::cabina::info_u64("linux", "unhandled syscall", nr as u64);
            -libc_consts::ENOSYS
        }
    }
}

fn handle_write(fd: i32, buf: *const u8, count: usize) -> i64 {
    if fd != 1 && fd != 2 {
        return -libc_consts::EBADF;
    }
    if buf.is_null() || count == 0 {
        return 0;
    }
    let slice = unsafe { core::slice::from_raw_parts(buf, count) };
    if let Ok(s) = core::str::from_utf8(slice) {
        crate::dev::console::serial_write(s);
    }
    count as i64
}

fn handle_exit(code: i32) -> i64 {
    crate::cabina::info_u64("linux", "exit", code as u64);
    loop { unsafe { core::arch::asm!("hlt"); } }
}

fn handle_exit_group(code: i32) -> i64 {
    crate::cabina::info_u64("linux", "exit_group", code as u64);
    loop { unsafe { core::arch::asm!("hlt"); } }
}

fn handle_brk(addr: *mut u8) -> i64 {
    // Simple brk: return current program break (stub)
    // Real implementation would track a program break per process
    crate::cabina::info_u64("linux", "brk stub", addr as u64);
    0x7F00_0000_0000 as i64
}

fn handle_mmap(addr: *mut u8, len: usize, _prot: i32, _flags: i32, _fd: i32, _off: isize) -> i64 {
    if !addr.is_null() {
        return addr as i64;
    }
    let align = 4096;
    let size = (len + align - 1) & !(align - 1);
    let layout = match core::alloc::Layout::from_size_align(size, align) {
        Ok(l) => l,
        Err(_) => return -libc_consts::EINVAL,
    };
    let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
    if ptr.is_null() {
        return -libc_consts::ENOMEM;
    }
    ptr as i64
}

fn handle_munmap(addr: *mut u8, len: usize) -> i64 {
    // No-op for now (we leak, acceptable in kernel
    0
}

fn handle_open(path: *const u8, _flags: i32, _mode: u16) -> i64 {
    // Stub: pretend the file exists and return a fake fd
    // A real implementation would parse the path and provide files
    let path_str = read_cstr(path);
    crate::cabina::info("linux", "open stub");
    let _ = path_str;
    -libc_consts::ENOENT
}

fn handle_read(_fd: i32, _buf: *mut u8, _count: usize) -> i64 {
    -libc_consts::ENOSYS
}

fn handle_close(_fd: i32) -> i64 {
    0
}

fn handle_getpid() -> i64 {
    1
}

fn handle_gettid() -> i64 {
    1
}

fn handle_nanosleep(_req: *const u8, _rem: *mut u8) -> i64 {
    0
}

fn handle_clock_gettime(_clk_id: u64, tp: *mut u8) -> i64 {
    if tp.is_null() {
        return -libc_consts::EFAULT;
    }
    // Return zero time for now
    unsafe {
        core::ptr::write_bytes(tp, 0, 16);
    }
    0
}

fn handle_uname(buf: *mut u8) -> i64 {
    if buf.is_null() {
        return -libc_consts::EFAULT;
    }
    let uts = b"Linux\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    unsafe {
        core::ptr::copy_nonoverlapping(uts.as_ptr(), buf, uts.len().min(65));
    }
    0
}

fn handle_getcwd(buf: *mut u8, size: usize) -> i64 {
    if buf.is_null() || size == 0 {
        return -libc_consts::EFAULT;
    }
    let cwd = b"/\x00";
    if cwd.len() > size {
        return -libc_consts::ERANGE;
    }
    unsafe { core::ptr::copy_nonoverlapping(cwd.as_ptr(), buf, cwd.len()); }
    cwd.len() as i64
}

fn handle_lseek(_fd: i32, _offset: isize, _whence: i32) -> i64 {
    0
}

fn handle_ioctl(_fd: i32, _request: u64, _argp: *mut u8) -> i64 {
    0
}

fn handle_access(_path: *const u8, _mode: i32) -> i64 {
    0 // pretend file exists
}

fn handle_fcntl(_fd: i32, _cmd: i32, _arg: u64) -> i64 {
    0
}

fn handle_fstat(_fd: i32, buf: *mut u8) -> i64 {
    if buf.is_null() {
        return -libc_consts::EFAULT;
    }
    unsafe { core::ptr::write_bytes(buf, 0, 144); }
    0
}

fn read_cstr(ptr: *const u8) -> core::result::Result<&'static str, ()> {
    if ptr.is_null() {
        return Err(());
    }
    let mut len = 0usize;
    while len < 256 {
        unsafe {
            if *ptr.add(len) == 0 {
                break;
            }
        }
        len += 1;
    }
    let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
    core::str::from_utf8(slice).map_err(|_| ())
}

mod libc_consts {
    pub const EPERM: i64 = 1;
    pub const ENOENT: i64 = 2;
    pub const ESRCH: i64 = 3;
    pub const EINTR: i64 = 4;
    pub const EIO: i64 = 5;
    pub const EBADF: i64 = 9;
    pub const ENOMEM: i64 = 12;
    pub const EACCES: i64 = 13;
    pub const EFAULT: i64 = 14;
    pub const ENODEV: i64 = 19;
    pub const EINVAL: i64 = 22;
    pub const ENOSYS: i64 = 38;
    pub const ERANGE: i64 = 34;
}
