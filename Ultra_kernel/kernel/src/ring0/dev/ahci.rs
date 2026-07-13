//! AHCI driver — stub.
//!
//! SATA/AHCI is deferred. No ports are enumerated in the Ring 0 base.

pub fn init() {}
pub fn port_count() -> usize { 0 }
