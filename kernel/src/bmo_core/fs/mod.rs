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
//!
//! v1.6.16: storage drivers (NVMe/AHCI/USB) are not wired because PCI
//! enumeration is skipped on this Ryzen 5 5600X. The DiskError enum
//! keeps all variants for the future drivers; only the ones the
//! RAMdisk uses are constructed.

#![allow(dead_code)]

pub mod inode;
pub mod mount;
pub mod manager;
pub mod fat32;
pub mod bmofs_loop;
pub mod ramdisk;

// ── Disk I/O traits (used by NVMe, AHCI, USB storage) ────────────────

/// Disk error type.
#[allow(dead_code)] // variants will be used by future disk drivers
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
    #[allow(dead_code)] // public API for future error reporting
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
#[allow(dead_code)] // write path is for future AHCI/NVMe drivers
pub trait DiskWriter: DiskReader {
    fn write_sectors(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<(), DiskError>;
}

/// Inicializa el subsistema de filesystem. v1.7.4: no-op
/// (los módulos se auto-inicializan con static lazy en sus BSS).
pub fn init() {
    // v2.0: registrar FAT32 read-only driver + BMO-FS read-write.
}

/// Capabilities de un proceso (qué puede hacer en el FS).
/// Usado por `proc::process` y `bef::manifest` para validar permisos.
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
    pub const FS_READ: Self = Self(1 << 0);   // alias
    pub const FS_WRITE: Self = Self(1 << 1);  // alias
    pub const SYS_TIME_HIRES: Self = Self(1 << 6);
    pub const SYS_GPU_SUBMIT: Self = Self(1 << 7);
    pub const SYS_INPUT: Self = Self(1 << 8);
    pub const NET_RAW: Self = Self(1 << 9);
    pub const ALL: Self = Self(0xFFFF_FFFF);

    pub fn has(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
    pub fn insert(&mut self, other: Self) { self.0 |= other.0; }
    pub fn remove(&mut self, other: Self) { self.0 &= !other.0; }
}
