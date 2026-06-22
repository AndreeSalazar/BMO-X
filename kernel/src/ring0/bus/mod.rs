//! `bus/` — System buses (ACPI, PCIe, MMIO, etc.).
//!
//! v1.8.8: re-exports `dev::acpi` and `dev::pcie` for the new path.
//! Future: this will be reorganized to host the actual bus drivers
//! (ACPI table parser, PCIe ECAM, etc.) with cleaner dependencies.

pub use crate::dev::acpi;
pub use crate::dev::pcie;
