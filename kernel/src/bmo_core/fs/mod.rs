//! FastOS/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! Filesystem subsystem for FastOS.
//!
//! v1.8.8: usa `BmoErrorCode` del ABI para errores visibles a Ring 3.
//! Antes tenía un `DiskError` local que no se podía mapear al ABI.
//!
//! Modular architecture (v1.8.8 simplificado):
//!   - VFS (Virtual File System) — unified API
//!   - Inode — file descriptor table (usa BmoFileType/BmoPerms)
//!   - Mount — mount point management
//!   - RAMdisk — embedded files in kernel binary (único FS en uso)
//!   - Disk traits — block I/O abstraction for drivers
//!
//! v1.8.8: FAT32 y exFAT se eliminaron. Eran código de preparación
//! sin disco físico. Cuando llegue el driver NVMe/AHCI en v1.9, se
//! reintroducen desde cero con la API actual.

#![allow(dead_code)]

pub mod inode;
pub mod mount;
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

pub use crate::bmo_abi::fs::Capabilities;

