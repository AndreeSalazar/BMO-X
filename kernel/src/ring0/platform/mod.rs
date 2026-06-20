//! Platform info — describes the hardware we're running on.
//!
//! In v1.7.5 this is mostly a stub. The long-term plan is:
//!   - `cpu`      — CPUID, vendor, family, model, features
//!   - `chipset`  — ACPI tables (RSDP, RSDT, XSDT, MCFG, MADT)
//!   - `firmware` — UEFI/BIOS runtime, boot services state
//!   - `topology` — NUMA, cores, packages
//!
//! For now, `features` lives in `crate::cpu` (re-exported here for
//! convenience) and ACPI tables live in `crate::dev::acpi` (control).
//! The split is deferred to v1.8.

#![allow(dead_code)]

pub mod cpu {
    pub use crate::cpu::features::{CpuFeatures, detect};
}

pub mod chipset {
    //! ACPI tables (RSDP, MCFG, etc).
    //!
    //! The control API (sleep, reboot) is in `crate::dev::acpi`.
    //! Here we only expose the parsed table snapshots.

    use crate::dev::acpi::{McfgHeader, RsdpHeader};

    pub fn mcfg() -> Option<McfgHeader> { crate::dev::acpi::mcfg_snapshot() }

    pub fn find_rsdp() -> u64 { crate::dev::acpi::find_rsdp() }

    pub fn parse_rsdp(addr: u64) -> Option<RsdpHeader> {
        crate::dev::acpi::parse_rsdp(addr)
    }
}

pub mod firmware {
    //! Firmware info. v1.7.5: UEFI-only, no real abstraction yet.
    pub fn is_uefi() -> bool { true }
    pub fn boot_services_active() -> bool { false }
}

pub mod topology {
    //! CPU topology. v1.7.5: single CPU, no NUMA.
    pub fn cpu_count() -> u32 { 1 }
    pub fn core_count() -> u32 { 1 }
    pub fn thread_count() -> u32 { 1 }
}
