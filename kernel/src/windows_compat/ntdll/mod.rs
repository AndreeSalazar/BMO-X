//! ntdll.dll — Gateway layer (like Wine's ntdll.dll).
//!
//! This is the LOWEST level of Windows compatibility. All Windows apps
//! eventually call ntdll.dll functions (NtCreateFile, NtReadFile, etc.).
//!
//! In Wine, ntdll.dll contains a syscall dispatcher that bridges the
//! PE/Windows world to the Unix/native world. In FastOS, we bridge
//! NT syscalls to BMO syscalls.
//!
//! Architecture:
//!   PE app → ntdll.dll (this module) → BMO syscall → kernel
//!
//! Key insight from Wine: ntdll.dll is the GATEWAY. Every Windows API
//! eventually routes through it. By implementing ntdll properly, we
//! get compatibility with thousands of Windows apps for free.

#![allow(dead_code)]

pub mod syscalls;
pub mod rtl;
pub mod memory;
pub mod file;
pub mod thread;
pub mod process;
pub mod objects;

/// Initialize ntdll compatibility layer.
pub fn init() {
    crate::diag::info("wcompat::ntdll", "NT gateway layer initialized");
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
    NotAllAssigned = 0x0000010A,
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
    pub fn is_success(self) -> bool {
        (self as i32) >= 0
    }
    pub fn is_error(self) -> bool {
        (self as i32) < 0
    }
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
