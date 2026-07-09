//! Shim module — Linux→BMO syscall translator.
//!
//! The shim is injected at each `syscall` site in the devoured ELF.
//! Instead of the Linux `syscall` instruction, the code calls our handler
//! which translates Linux syscall numbers to BMO syscall numbers.

/// Linux syscall → BMO syscall translation.
/// Returns the BMO syscall number, or u64::MAX for unsupported.
pub fn translate_linux_to_bmo(linux_nr: u64) -> u64 {
    match linux_nr {
        // Nivel 1: CLI básico
        1   => 0xF0, // write → debug_print
        9   => 0x10, // mmap → MMAP
        12  => 0x10, // brk → MMAP (heap)
        60  => 0x00, // exit → EXIT
        231 => 0x00, // exit_group → EXIT

        // Nivel 2: Filesystem
        2   => 0x20, // open
        3   => 0x24, // close
        257 => 0x20, // openat
        5   => 0x23, // fstat
        8   => 0x24, // lseek
        78  => 0x22, // getdents64

        // Nivel 3: Extra
        0   => 0xF0, // read → debug (stdin stub)
        11  => 0x03, // sched_yield → yield
        35  => 0x51, // nanosleep
        39  => 0x03, // getpid → stub
        102 => 0x50, // getuid → time (stub)
        201 => 0x50, // time → clock_get
        228 => 0x50, // clock_gettime → clock_get

        _ => u64::MAX, // unsupported → ENOSYS
    }
}

/// Get the human-readable name of a Linux syscall (for logs).
pub fn linux_syscall_name(nr: u64) -> &'static str {
    match nr {
        0 => "read", 1 => "write", 2 => "open", 3 => "close",
        5 => "fstat", 8 => "lseek", 9 => "mmap", 11 => "sched_yield",
        12 => "brk", 35 => "nanosleep", 39 => "getpid",
        60 => "exit", 78 => "getdents64", 102 => "getuid",
        201 => "time", 228 => "clock_gettime", 231 => "exit_group",
        257 => "openat",
        _ => "unknown",
    }
}
