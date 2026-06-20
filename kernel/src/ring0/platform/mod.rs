//! Platform info — describes the hardware we're running on.
//!
//! In v1.7.6 this is mostly a stub. The long-term plan is:
//!   - `cpu`      — CPUID, vendor, family, model, features
//!   - `chipset`  — ACPI tables (RSDP, RSDT, XSDT, MCFG, MADT)
//!   - `firmware` — UEFI/BIOS runtime, boot services state
//!   - `topology` — NUMA, cores, packages
//!
//! v1.7.6: `cpu` is fully implemented (Ryzen 5 5600X optimized but
//! portable to other x86-64 CPUs). The rest is stubbed and will be
//! filled in v1.8.

#![allow(dead_code)]

pub mod cpu;
pub use cpu::{CpuIdentity, Vendor, Microarch, FeatureBitmap, CacheInfo, detect as detect_cpu};

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
    //! Firmware info. v1.7.6: UEFI-only, no real abstraction yet.
    pub fn is_uefi() -> bool { true }
    pub fn boot_services_active() -> bool { false }
}

pub mod topology {
    //! CPU topology. v1.7.6: detected via `arch::topology::detect`.
    pub use crate::arch::topology::{Topology, CpuId, PerCpu};
    pub use crate::arch::topology::{detect, bsp_apic_id, init, online_count};
    pub fn cpu_count() -> u32 { crate::arch::topology::online_count() }
}
