//! ntdll.dll — NT gateway layer (analogous to Wine's ntdll.dll).
//!
//! The gateway: every Windows API eventually routes through ntdll.
//! In BMO, the ntdll gateway translates NT syscalls to BMO syscalls.
//!
//! Architecture:
//! ```text
//!   PE app → ntdll.dll (this module) → BMO syscall → kernel
//! ```
//!
//! Sub-files in this directory:
//!   - `ntdll_syscalls.rs` — the actual NT function implementations
//!   - `ntdll_memory.rs`   — Nt* memory syscalls (Zw* variants too)
//!   - `ntdll_file.rs`     — Nt* file syscalls
//!
//! (No more rtl, objects, thread, process: those are covered by the
//! kernel32 wrapper sub-files. Eliminated as legacy.)

#![allow(dead_code)]

/// Initialize the ntdll gateway.
pub fn init() {
    crate::bmo_core::diag::info("bmo_abi::interop::win32::ntdll", "NT gateway layer initialized");
}

/// NT status codes (like Windows NTSTATUS).
///
/// Only unique values — duplicates removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NtStatus {
    Success = 0,
    Abandoned = 0x00000080,
    Alerted = 0x00000101,
    Timeout = 0x00000102,
    Pending = 0x00000103,
    MoreEntries = 0x00000105,
    NotAllAssigned = 0x00000010A,
    SomeNotMapped = 0x0000010B,
    Unsuccessful = -1,           // 0xC0000001
    NotImplemented = -2,         // 0xC0000002
    InvalidHandle = -8,          // 0xC0000008
    EndOfFile = -15,             // 0xC0000011
    NoSuchFile = -16,            // 0xC000000F
    NoMemory = -23,              // 0xC0000017
    ConflictingAddresses = -24,  // 0xC0000018
    UnableToFreeVirtualMemory = -26, // 0xC000001A
    AccessDenied = -62,          // 0xC0000022
    BufferTooSmall = -63,        // 0xC0000023
    ObjectNameNotFound = -76,    // 0xC0000034
    InvalidParameter = -93,      // 0xC000000D (adjusted to be unique)
    AccountRestriction = -107,   // 0xC000006B (adjusted to be unique)
    InsufficientResources = -154, // 0xC000009A (adjusted to be unique)
    FileIsADirectory = -186,     // 0xC00000BA (adjusted to be unique)
}

impl NtStatus {
    pub fn is_success(self) -> bool { (self as i32) >= 0 }
    pub fn is_error(self) -> bool { (self as i32) < 0 }
}

/// NT object attributes (simplified).
#[repr(C)]
pub struct ObjectAttributes {
    pub length: u32,
    pub root_directory: u64,
    pub object_name: u64,      // UNICODE_STRING*
    pub attributes: u32,
    pub security_descriptor: u64,
    pub security_quality_of_service: u64,
}

/// NT IO status block.
#[repr(C)]
pub struct IoStatusBlock {
    pub status: i32,
    pub information: u64,
}

/// NT large integer (64-bit).
#[repr(C)]
pub struct LargeInteger {
    pub quad_part: i64,
}
