//! `fs` — sólo los traits/error que necesitan los drivers de disco.
//!
//! El antiguo árbol completo (ntfs/walker/gpt + crate `ntfs`/`nt-hive`/`binrw`)
//! salió del kernel cuando el modo "Spy Agent" se abandonó. Estos shims
//! existen porque `drivers/nvme.rs`, `drivers/ahci.rs` y
//! `drivers/gpu/fastgpu/gsp` aún implementan estos traits.

#![allow(dead_code)]

pub trait DiskReader {
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError>;
}

pub trait DiskWriter {
    fn write_sectors(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<(), DiskError>;
}

#[derive(Debug, Clone, Copy)]
pub enum DiskError {
    ControllerError,
    InvalidLba,
    Timeout,
    IOError,
}
