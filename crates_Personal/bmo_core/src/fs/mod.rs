//! BMO/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! Filesystem subsystem for BMO.
//!
//! v1.8.8: usa `BmoErrorCode` del ABI para errores visibles a Ring 3.
//! Antes tenÃ­a un `DiskError` local que no se podÃ­a mapear al ABI.
//!
//! Modular architecture (v1.8.8 simplificado):
//!   - VFS (Virtual File System) â€” unified API
//!   - Inode â€” file descriptor table (usa BmoFileType/BmoPerms)
//!   - Mount â€” mount point management
//!   - RAMdisk â€” embedded files in kernel binary (Ãºnico FS en uso)
//!   - Disk traits â€” block I/O abstraction for drivers
//!
//! v1.8.8: exFAT driver en vendor crate (para particiÃ³n data T:).

#![allow(dead_code)]

pub mod inode;
pub mod mount;
pub mod ramdisk;
pub mod ramdisk_device;

use crate::bmo_abi::error_code::BmoErrorCode;

/// Disk error â€” re-export del ABI. BmoErrorCode tiene 21 cÃ³digos que
/// cubren todos los errores de disk (NotFound, Timeout, Io, etc).
///
/// Los drivers de bajo nivel pueden mapear sus errores especÃ­ficos
/// a BmoErrorCode en el punto donde cruzan la frontera ring 0â†”ring 3.
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
    // exFAT (data, T:) initialized on-demand when AHCI driver is wired.
    // Ramdisk is always available.
}

pub use crate::bmo_abi::fs::Capabilities;

