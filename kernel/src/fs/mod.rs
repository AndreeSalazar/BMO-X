//! Filesystem subsystem for FastOS.
//!
//! Modular architecture:
//!   - VFS (Virtual File System) — unified API
//!   - Inode — file descriptor table
//!   - Mount — mount point management
//!   - FAT32 — FAT32 read-only filesystem
//!   - BMO-FS — native filesystem (via ramdisk or disk)
//!   - RAMdisk — embedded files in kernel binary
//!   - Disk traits — block I/O abstraction for drivers

pub mod inode;
pub mod mount;
pub mod manager;
pub mod fat32;
pub mod bmofs_loop;
pub mod ramdisk;

// ── Disk I/O traits (used by NVMe, AHCI, USB storage) ────────────────

/// Disk error type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiskError {
    NotFound,
    Timeout,
    IOError,
    InvalidLba,
    NoMedia,
    BadBlock,
    Uninitialized,
}

impl DiskError {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiskError::NotFound => "not found",
            DiskError::Timeout => "timeout",
            DiskError::IOError => "I/O error",
            DiskError::InvalidLba => "invalid LBA",
            DiskError::NoMedia => "no media",
            DiskError::BadBlock => "bad block",
            DiskError::Uninitialized => "uninitialized",
        }
    }
}

/// Read-only block device trait.
pub trait DiskReader {
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError>;
}

/// Read-write block device trait.
pub trait DiskWriter: DiskReader {
    fn write_sectors(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<(), DiskError>;
}
