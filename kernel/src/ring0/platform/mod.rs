//! Platform info for the Ryzen 5 5600X.
//!
//! v1.7.8: simplified. `cpu::detect()` returns CpuIdentity with
//! vendor/family/model/features/cache/topology. All values are
//! hardcoded for the 5600X — if you change CPU, edit this file.

#![allow(dead_code)]

pub mod cpu;
pub use cpu::{CpuIdentity, Vendor, Microarch, detect as detect_cpu};

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
    //! Firmware info. v1.7.8: UEFI-only, no real abstraction yet.
    pub fn is_uefi() -> bool { true }
    pub fn boot_services_active() -> bool { false }
}

pub mod topology {
    //! CPU topology. v1.7.8: re-export from arch::topology.
    pub use crate::arch::topology::{
        Topology, CpuId, PerCpu,
        detect, bsp_apic_id, init, online_count,
    };
    pub fn cpu_count() -> u32 { crate::arch::topology::online_count() }
}

/// Re-export the 5600X topology constants. Always-true for the 5600X.
pub mod r5_5600x {
    pub const TOTAL_THREADS: u32 = 12;
    pub const CORE_COUNT: u32 = 6;
    pub const CCD_COUNT: u32 = 1;
    pub const THREADS_PER_CORE: u8 = 2;
    pub const CORES_PER_CCD: u8 = 6;
    pub const TSC_BASE_HZ: u64 = 3_700_000_000;
    pub const TSC_BOOST_HZ: u64 = 4_850_000_000;
    pub const L1_SIZE_KB: u32 = 32;
    pub const L2_SIZE_KB: u32 = 512;
    pub const L3_SIZE_KB: u32 = 32_768;
    pub const VIRT_ADDR_BITS: u8 = 48;
    pub const PHYS_ADDR_BITS: u8 = 40;
}
