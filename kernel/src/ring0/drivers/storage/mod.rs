#![allow(dead_code)]

//! Unified storage subsystem for FastOS.
//!
//! Provides a single entry point for disk I/O that routes to the best
//! available backend:
//!   1. NVMe (preferred — fastest)
//!   2. AHCI/SATA (fallback)
//!   3. RAM fallback (for testing)
//!
//! Individual drivers implement the raw block I/O; this module integrates them.

use crate::drivers::serial;

/// Active storage backend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StorageBackend {
    None,
    Nvme,
    Ahci,
    Ram,
}

/// Storage statistics.
pub struct StorageStats {
    pub reads: u64,
    pub writes: u64,
    pub errors: u64,
    pub backend: StorageBackend,
}

pub static mut STATS: StorageStats = StorageStats {
    reads: 0,
    writes: 0,
    errors: 0,
    backend: StorageBackend::None,
};

/// Initialize the storage subsystem.
/// Detects the best available backend.
pub fn init() -> StorageBackend {
    unsafe {
        if crate::drivers::nvme::NVME_DRIVER.is_some() {
            STATS.backend = StorageBackend::Nvme;
            serial::serial_write("[storage] NVMe backend active\n");
            return StorageBackend::Nvme;
        }

        if crate::drivers::ahci::AHCI_DRIVER.is_some() {
            STATS.backend = StorageBackend::Ahci;
            serial::serial_write("[storage] AHCI/SATA backend active\n");
            return StorageBackend::Ahci;
        }

        STATS.backend = StorageBackend::Ram;
        serial::serial_write("[storage] RAM fallback active\n");
        StorageBackend::Ram
    }
}

/// Get the current storage backend.
pub fn backend() -> StorageBackend {
    unsafe { STATS.backend }
}

/// Backend name string.
pub fn backend_name() -> &'static str {
    match unsafe { STATS.backend } {
        StorageBackend::Nvme => "NVMe",
        StorageBackend::Ahci => "AHCI/SATA",
        StorageBackend::Ram => "RAM",
        StorageBackend::None => "None",
    }
}

/// Print storage status to serial.
pub fn print_status() {
    serial::serial_write("[storage] Backend: ");
    serial::serial_write(backend_name());
    let s = unsafe { &STATS };
    serial::serial_write(" reads=");
    serial_write_u64(s.reads);
    serial::serial_write(" writes=");
    serial_write_u64(s.writes);
    serial::serial_write(" errors=");
    serial_write_u64(s.errors);
    serial::serial_write("\n");
}

fn serial_write_u64(val: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = val;
    if v == 0 { i -= 1; buf[i] = b'0'; }
    else { while v > 0 { i -= 1; buf[i] = b'0' + (v % 10) as u8; v /= 10; } }
    serial::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}
