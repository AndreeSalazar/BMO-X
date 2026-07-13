//! Storage driver — stub.
//!
//! AHCI / NVMe / USB mass storage are deferred. The boot chain's
//! `stage3_dev` already provides a basic ramdisk via the UEFI chain.

pub fn init() {}
pub fn is_ready() -> bool { false }
pub fn port_count() -> usize { 0 }
