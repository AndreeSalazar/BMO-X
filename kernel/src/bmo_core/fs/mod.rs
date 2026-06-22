//! Filesystem subsystem for FastOS.
//!
//! v1.8.8: usa `BmoErrorCode` del ABI para errores visibles a Ring 3.
//! Antes tenía un `DiskError` local que no se podía mapear al ABI.
//!
//! Modular architecture:
//!   - VFS (Virtual File System) — unified API
//!   - Inode — file descriptor table (usa BmoFileType/BmoPerms)
//!   - Mount — mount point management
//!   - FAT32 — boot partition (UEFI compatible, read-only)
//!   - exFAT — data partition (read-write, modern)
//!   - RAMdisk — embedded files in kernel binary
//!   - Disk traits — block I/O abstraction for drivers

#![allow(dead_code)]

pub mod inode;
pub mod mount;
pub mod manager;
pub mod fat32;
pub mod exfat;
pub mod ramdisk;
pub mod ramdisk_device;

use crate::bmo_abi::error_code::BmoErrorCode;

/// Disk error — re-export del ABI. BmoErrorCode tiene 21 códigos que
/// cubren todos los errores de disk (NotFound, Timeout, Io, etc).
///
/// Los drivers de bajo nivel pueden mapear sus errores específicos
/// a BmoErrorCode en el punto donde cruzan la frontera ring 0↔ring 3.
pub type DiskError = BmoErrorCode;

/// Read-only block device trait.
pub trait DiskReader {
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError>;
}

/// Read-write block device trait.
pub trait DiskWriter: DiskReader {
    fn write_sectors(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<(), DiskError>;
}

/// Inicializa el subsistema de filesystem.
pub fn init() {
    // FAT32 (boot) and exFAT (data) are initialized on-demand when
    // a disk driver is wired. Ramdisk is always available.
}

/// Capabilities de un proceso (qué puede hacer en el FS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities(pub u32);

impl Capabilities {
    pub const NONE: Self = Self(0);
    pub const READ_FS: Self = Self(1 << 0);
    pub const WRITE_FS: Self = Self(1 << 1);
    pub const EXEC: Self = Self(1 << 2);
    pub const NET: Self = Self(1 << 3);
    pub const GPU: Self = Self(1 << 4);
    pub const SYS_DEBUG: Self = Self(1 << 5);
    pub const FS_READ: Self = Self(1 << 0);
    pub const FS_WRITE: Self = Self(1 << 1);
    pub const SYS_TIME_HIRES: Self = Self(1 << 6);
    pub const SYS_GPU_SUBMIT: Self = Self(1 << 7);
    pub const SYS_INPUT: Self = Self(1 << 8);
    pub const NET_RAW: Self = Self(1 << 9);
    pub const ALL: Self = Self(0xFFFF_FFFF);

    pub fn has(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
    pub fn insert(&mut self, other: Self) { self.0 |= other.0; }
    pub fn remove(&mut self, other: Self) { self.0 &= !other.0; }
}
