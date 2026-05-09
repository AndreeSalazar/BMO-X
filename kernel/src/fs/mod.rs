//! File System abstractions for FastOS.

pub mod ntfs;
pub mod walker;

/// Abstract block access to a disk device.
pub trait DiskReader {
    /// Read `count` sectors starting at `lba` into `buf`.
    /// `buf` must be at least `count * 512` bytes.
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError>;
}

#[derive(Debug, Clone, Copy)]
pub enum DiskError {
    ControllerError,
    InvalidLba,
    Timeout,
    IOError,
}
