// Linux x86_64 syscall dispatch.
//
// Cada handler recibe (a0..a5: u64) y hace su propio casting.
// El dispatch es una tabla plana — un solo match, sin conversiones.
//
// Las constantes SYS_* se usan desde libc.rs para invocar dispatch(),
// y también desde el dispatch directo en ring0/arch/syscall.rs.

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
pub const SYS_SCHED_YIELD: usize = 24;
pub const SYS_DUP: usize = 32;
pub const SYS_DUP2: usize = 33;
pub const SYS_NANOSLEEP: usize = 35;
pub const SYS_GETPID: usize = 39;
pub const SYS_SENDFILE: usize = 40;
pub const SYS_EXIT: usize = 60;
pub const SYS_UNAME: usize = 63;
pub const SYS_FCNTL: usize = 72;
pub const SYS_TRUNCATE: usize = 76;
pub const SYS_FTRUNCATE: usize = 77;
pub const SYS_GETDENTS: usize = 78;
pub const SYS_GETCWD: usize = 79;
pub const SYS_CHDIR: usize = 80;
pub const SYS_MKDIR: usize = 83;
pub const SYS_RMDIR: usize = 84;
pub const SYS_LINK: usize = 86;
pub const SYS_UNLINK: usize = 87;
pub const SYS_SYMLINK: usize = 88;
pub const SYS_READLINK: usize = 89;
pub const SYS_GETTIMEOFDAY: usize = 96;
pub const SYS_GETTID: usize = 186;
pub const SYS_FUTEX: usize = 202;
pub const SYS_SET_TID_ADDRESS: usize = 218;
pub const SYS_CLOCK_GETTIME: usize = 228;
pub const SYS_EXIT_GROUP: usize = 231;
pub const SYS_SET_ROBUST_LIST: usize = 273;
pub const SYS_GETRANDOM: usize = 318;

use super::{errno, fs, mem, proc, sync, time};

type H = fn(u64, u64, u64, u64, u64, u64) -> i64;

fn stub(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> i64 {
    -errno::ENOSYS
}

pub fn dispatch(nr: usize, args: &[u64; 6]) -> i64 {
    let h: H = match nr {
        0   => fs::sys_read,
        1   => fs::sys_write,
        2   => fs::sys_open,
        3   => fs::sys_close,
        4   => stub, // stat
        5   => fs::sys_fstat,
        8   => fs::sys_lseek,
        9   => mem::sys_mmap,
        10  => mem::sys_mprotect,
        11  => mem::sys_munmap,
        12  => mem::sys_brk,
        13  => stub, // rt_sigaction
        14  => stub, // rt_sigprocmask
        16  => fs::sys_ioctl,
        19  => stub, // readv
        20  => stub, // writev
        21  => fs::sys_access,
        22  => stub, // pipe
        24  => proc::sys_sched_yield,
        32  => stub, // dup
        33  => stub, // dup2
        35  => time::sys_nanosleep,
        39  => proc::sys_getpid,
        40  => stub, // sendfile
        60  => proc::sys_exit,
        63  => proc::sys_uname,
        72  => fs::sys_fcntl,
        76  => stub, // truncate
        77  => stub, // ftruncate
        78  => stub, // getdents
        79  => proc::sys_getcwd,
        80  => stub, // chdir
        83  => stub, // mkdir
        84  => stub, // rmdir
        86  => stub, // link
        87  => stub, // unlink
        88  => stub, // symlink
        89  => stub, // readlink
        96  => time::sys_gettimeofday,
        186 => proc::sys_gettid,
        202 => sync::sys_futex,
        218 => proc::sys_set_tid_address,
        228 => time::sys_clock_gettime,
        231 => proc::sys_exit_group,
        273 => proc::sys_set_robust_list,
        318 => proc::sys_getrandom,
        _   => return {
            crate::cabina::info_u64("linux", "unhandled syscall", nr as u64);
            -errno::ENOSYS
        },
    };
    h(args[0], args[1], args[2], args[3], args[4], args[5])
}
