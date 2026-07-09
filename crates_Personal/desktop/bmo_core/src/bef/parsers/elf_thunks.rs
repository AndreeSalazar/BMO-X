//! Fake-libs Linux/Unix â€” tabla de funciones que el devour-loader provee a
//! binarios ELF para que **crean** que estÃ¡n corriendo sobre Linux/glibc.
//!
//! Cuando un ELF importa `libc.so.6!malloc`, el resolver busca aquÃ­ y le
//! da un puntero a un wrapper Rust que traduce a allocator BMO / BMO.

#![allow(dead_code)]

use crate::bmo_abi::primitives::bx_u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfThunkTarget {
    SilentStub,
    LogStub,
    BarexGraphics,
    BarexAudio,
    BarexInput,
    BarexNet,
    SyscallVfs,
    SyscallProcess,
    SyscallTime,
    SyscallMemory,
    SyscallFutex,
    LibmMath,
    LibcStringOps,
}

#[derive(Debug, Clone, Copy)]
pub struct ElfThunkEntry {
    pub lib: &'static str,
    pub name: &'static str,
    pub target: ElfThunkTarget,
}

/// Tabla maestra. **Nombres canÃ³nicos sin versiones** (`libc.so.6` se
/// normaliza a `libc.so` en el resolver).
pub static THUNK_TABLE: &[ElfThunkEntry] = &[
    // â”€â”€â”€ libc.so (proceso, memoria, archivos, tiempo) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    ElfThunkEntry { lib: "libc.so", name: "exit",            target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "_exit",           target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "abort",           target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "getpid",          target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "gettid",          target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "fork",            target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "execve",          target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "wait",            target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "waitpid",         target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "pthread_create",  target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "pthread_join",    target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libc.so", name: "pthread_mutex_init",   target: ElfThunkTarget::SyscallFutex },
    ElfThunkEntry { lib: "libc.so", name: "pthread_mutex_lock",   target: ElfThunkTarget::SyscallFutex },
    ElfThunkEntry { lib: "libc.so", name: "pthread_mutex_unlock", target: ElfThunkTarget::SyscallFutex },

    // memoria
    ElfThunkEntry { lib: "libc.so", name: "malloc",          target: ElfThunkTarget::SyscallMemory },
    ElfThunkEntry { lib: "libc.so", name: "calloc",          target: ElfThunkTarget::SyscallMemory },
    ElfThunkEntry { lib: "libc.so", name: "realloc",         target: ElfThunkTarget::SyscallMemory },
    ElfThunkEntry { lib: "libc.so", name: "free",            target: ElfThunkTarget::SyscallMemory },
    ElfThunkEntry { lib: "libc.so", name: "mmap",            target: ElfThunkTarget::SyscallMemory },
    ElfThunkEntry { lib: "libc.so", name: "munmap",          target: ElfThunkTarget::SyscallMemory },
    ElfThunkEntry { lib: "libc.so", name: "mprotect",        target: ElfThunkTarget::SyscallMemory },
    ElfThunkEntry { lib: "libc.so", name: "brk",             target: ElfThunkTarget::SyscallMemory },
    ElfThunkEntry { lib: "libc.so", name: "sbrk",            target: ElfThunkTarget::SyscallMemory },
    ElfThunkEntry { lib: "libc.so", name: "posix_memalign",  target: ElfThunkTarget::SyscallMemory },

    // archivos
    ElfThunkEntry { lib: "libc.so", name: "open",            target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "openat",          target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "close",           target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "read",            target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "write",           target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "lseek",           target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "fstat",           target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "stat",            target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "lstat",           target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "unlink",          target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "rename",          target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "fopen",           target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "fclose",          target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "fread",           target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "fwrite",          target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "fseek",           target: ElfThunkTarget::SyscallVfs },
    ElfThunkEntry { lib: "libc.so", name: "fprintf",         target: ElfThunkTarget::LogStub },
    ElfThunkEntry { lib: "libc.so", name: "printf",          target: ElfThunkTarget::LogStub },
    ElfThunkEntry { lib: "libc.so", name: "puts",            target: ElfThunkTarget::LogStub },

    // tiempo
    ElfThunkEntry { lib: "libc.so", name: "time",            target: ElfThunkTarget::SyscallTime },
    ElfThunkEntry { lib: "libc.so", name: "gettimeofday",    target: ElfThunkTarget::SyscallTime },
    ElfThunkEntry { lib: "libc.so", name: "clock_gettime",   target: ElfThunkTarget::SyscallTime },
    ElfThunkEntry { lib: "libc.so", name: "nanosleep",       target: ElfThunkTarget::SyscallTime },
    ElfThunkEntry { lib: "libc.so", name: "usleep",          target: ElfThunkTarget::SyscallTime },
    ElfThunkEntry { lib: "libc.so", name: "sleep",           target: ElfThunkTarget::SyscallTime },

    // strings
    ElfThunkEntry { lib: "libc.so", name: "strlen",          target: ElfThunkTarget::LibcStringOps },
    ElfThunkEntry { lib: "libc.so", name: "strcpy",          target: ElfThunkTarget::LibcStringOps },
    ElfThunkEntry { lib: "libc.so", name: "strncpy",         target: ElfThunkTarget::LibcStringOps },
    ElfThunkEntry { lib: "libc.so", name: "strcmp",          target: ElfThunkTarget::LibcStringOps },
    ElfThunkEntry { lib: "libc.so", name: "strncmp",         target: ElfThunkTarget::LibcStringOps },
    ElfThunkEntry { lib: "libc.so", name: "memcpy",          target: ElfThunkTarget::LibcStringOps },
    ElfThunkEntry { lib: "libc.so", name: "memset",          target: ElfThunkTarget::LibcStringOps },
    ElfThunkEntry { lib: "libc.so", name: "memcmp",          target: ElfThunkTarget::LibcStringOps },
    ElfThunkEntry { lib: "libc.so", name: "memmove",         target: ElfThunkTarget::LibcStringOps },

    // â”€â”€â”€ libm.so (math) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    ElfThunkEntry { lib: "libm.so", name: "sin",             target: ElfThunkTarget::LibmMath },
    ElfThunkEntry { lib: "libm.so", name: "cos",             target: ElfThunkTarget::LibmMath },
    ElfThunkEntry { lib: "libm.so", name: "tan",             target: ElfThunkTarget::LibmMath },
    ElfThunkEntry { lib: "libm.so", name: "sqrt",            target: ElfThunkTarget::LibmMath },
    ElfThunkEntry { lib: "libm.so", name: "pow",             target: ElfThunkTarget::LibmMath },
    ElfThunkEntry { lib: "libm.so", name: "log",             target: ElfThunkTarget::LibmMath },
    ElfThunkEntry { lib: "libm.so", name: "exp",             target: ElfThunkTarget::LibmMath },
    ElfThunkEntry { lib: "libm.so", name: "floor",           target: ElfThunkTarget::LibmMath },
    ElfThunkEntry { lib: "libm.so", name: "ceil",            target: ElfThunkTarget::LibmMath },

    // â”€â”€â”€ libpthread.so â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    ElfThunkEntry { lib: "libpthread.so", name: "pthread_create",   target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libpthread.so", name: "pthread_join",     target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libpthread.so", name: "pthread_self",     target: ElfThunkTarget::SyscallProcess },
    ElfThunkEntry { lib: "libpthread.so", name: "pthread_exit",     target: ElfThunkTarget::SyscallProcess },

    // â”€â”€â”€ libdl.so (dlopen/dlsym) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    ElfThunkEntry { lib: "libdl.so", name: "dlopen",         target: ElfThunkTarget::SilentStub },
    ElfThunkEntry { lib: "libdl.so", name: "dlsym",          target: ElfThunkTarget::SilentStub },
    ElfThunkEntry { lib: "libdl.so", name: "dlclose",        target: ElfThunkTarget::SilentStub },
];

/// Resuelve `(lib_name, fn_name)`. El `lib_name` puede venir como
/// `libc.so.6`, `libc-2.31.so`, etc.; lo normalizamos al canÃ³nico antes.
pub fn resolve(lib: &str, name: &str) -> ElfThunkTarget {
    let normalized = normalize_lib_name(lib);
    for e in THUNK_TABLE {
        if e.lib == normalized && e.name == name {
            return e.target;
        }
    }
    ElfThunkTarget::SilentStub
}

pub const fn thunk_table_len() -> usize { THUNK_TABLE.len() }

/// Normaliza `libc.so.6` / `libc-2.31.so` / `/lib64/libc.so.6` â†’ `libc.so`.
pub fn normalize_lib_name(lib: &str) -> &'static str {
    let basename = lib.rsplit('/').next().unwrap_or(lib);
    if basename.starts_with("libc")        { return "libc.so"; }
    if basename.starts_with("libm")        { return "libm.so"; }
    if basename.starts_with("libpthread")  { return "libpthread.so"; }
    if basename.starts_with("libdl")       { return "libdl.so"; }
    if basename.starts_with("librt")       { return "librt.so"; }
    "<unknown>"
}

#[allow(unused)]
pub extern "C" fn silent_stub() -> bx_u64 { 0 }

/// Resuelve un sÃ­mbolo a un puntero de funciÃ³n real.
/// Busca en los shims de linux/win32 segÃºn el lib_name.
pub fn resolve_fn_ptr(lib: &str, name: &str) -> Option<*const ()> {
    let normalized = normalize_lib_name(lib);
    match normalized {
        "libc.so" => resolve_libc(name),
        _ => None,
    }
}

fn resolve_libc(name: &str) -> Option<*const ()> {
    // Each entry maps to the extern "C" function in shims::linux::libc
    match name {
        "write" => Some(crate::bef::shims::linux::libc::write as *const ()),
        "read" => Some(crate::bef::shims::linux::libc::read as *const ()),
        "open" => Some(crate::bef::shims::linux::libc::open as *const ()),
        "close" => Some(crate::bef::shims::linux::libc::close as *const ()),
        "exit" | "_exit" => Some(crate::bef::shims::linux::libc::exit as *const ()),
        "exit_group" => Some(crate::bef::shims::linux::libc::exit_group as *const ()),
        "mmap" => Some(crate::bef::shims::linux::libc::mmap as *const ()),
        "munmap" => Some(crate::bef::shims::linux::libc::munmap as *const ()),
        "brk" => Some(crate::bef::shims::linux::libc::brk as *const ()),
        "getpid" => Some(crate::bef::shims::linux::libc::getpid as *const ()),
        "gettid" => Some(crate::bef::shims::linux::libc::gettid as *const ()),
        "nanosleep" => Some(crate::bef::shims::linux::libc::nanosleep as *const ()),
        "clock_gettime" => Some(crate::bef::shims::linux::libc::clock_gettime as *const ()),
        "uname" => Some(crate::bef::shims::linux::libc::uname as *const ()),
        "getcwd" => Some(crate::bef::shims::linux::libc::getcwd as *const ()),
        "lseek" => Some(crate::bef::shims::linux::libc::lseek as *const ()),
        "ioctl" => Some(crate::bef::shims::linux::libc::ioctl as *const ()),
        "access" => Some(crate::bef::shims::linux::libc::access as *const ()),
        "fcntl" => Some(crate::bef::shims::linux::libc::fcntl as *const ()),
        "fstat" => Some(crate::bef::shims::linux::libc::fstat as *const ()),
        "sched_yield" => Some(crate::bef::shims::linux::libc::sched_yield as *const ()),
        _ => {
            crate::cabina::info_u64("elf", "unresolved libc symbol: ", 0);
            crate::cabina::info("elf", name);
            Some(silent_stub as *const ())
        }
    }
}


