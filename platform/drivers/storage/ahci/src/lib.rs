//! BMO AHCI/SATA Storage Driver -- HAL-based.
//!
//! Implements controller detection, port enumeration, DMA setup,
//! and sector read/write. Uses `StorageHal` trait for kernel services
//! (memory allocation and logging).

#![no_std]

pub mod storage_hal;
pub mod controller;

pub use storage_hal::StorageHal;
pub use controller::{AhciController, PortState, AhciPort, DiskError, read_sectors_phys, write_sectors_phys, flush_cache, identify_phys, probe, init_port_dma, controller, reset_ctrl, SIG_SATA_DISK, SECTOR};
