//! Filesystem Service — Virtual File System stub.
//!
//! When AHCI driver is available, this layer will mount
//! FAT32/exFAT partitions and provide file I/O.

#![no_std]

/// FS error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    PermissionDenied,
    NotSupported,
    IoError,
}

/// Currently only ramdisk is available.
/// TODO: Wire AHCI → FAT32/exFAT → VFS.
pub fn init() {
    // Stub: ramdisk already initialized by kernel
}
