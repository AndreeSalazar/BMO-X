#![no_std]

//! FastOS Hardware Abstraction Layer
//!
//! Modular vendor crate providing:
//! - PCI config space access (IO ports + ECAM)
//! - AHCI/SATA disk driver
//! - Block device abstraction
//! - exFAT filesystem
//! - Kernel logging to SSD

extern crate alloc;

pub mod pci;
pub mod ahci;
pub mod storage;
pub mod fs;
pub mod log;
