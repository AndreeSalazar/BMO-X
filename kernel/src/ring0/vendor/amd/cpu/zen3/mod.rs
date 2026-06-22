//! `vendor/amd/cpu/zen3/` — Knowledge of the AMD Ryzen 5 5600X (Zen 3).
//!
//! v1.8.8 (Phase 2): The actual code is now HERE. It was moved from
//! `kernel/src/ring0/AMD/zen3/` to this new path. The `crate::amd_cpu`
//! alias is preserved for backwards compatibility.
//!
//! ## What lives here
//!
//! All knowledge specific to the AMD Ryzen 5 5600X (Vermeer, Zen 3,
//! Family 19h Model 01h). Nothing in this directory should be usable
//! on a different CPU — those would live in `vendor/amd/cpu/zen4/`
//! (future) or `vendor/intel/...` (future).
//!
//! ## Submodules
//!
//! - `cpuid_detection` — Family/model/feature detection
//! - `topology` — SMT/CCX/CCD/APIC IDs
//! - `cache_topology` — L1/L2/L3/TLB sizes
//! - `memory_ordering` — TSO débil, fences, atomic ops
//! - `msr_definitions` — Tabla MSRs
//! - `msr_init` — init_msr_common (EFER, STAR, LSTAR, FMASK, PAT, etc.)
//! - `tsc_calibration` — Calibración con PM Timer
//! - `power_management` — C1, C1e, P-state query
//! - `mtrr_pat` — MTRR + PAT configuration
//! - `errata_workarounds` — Spectre v2/v4, MDS, IBPB
//! - `model_comparison` — Zen 1/2/3/4/5 differences
//! - `acpi_real` — RSDP, XSDT, MCFG, FADT parsing
//! - `fastos_cpu` — API pública consolidada

#![allow(dead_code)] // v1.8.8: many helpers, not all called from ring0 yet

// ── Submodules (the actual code) ───────────────────────────────────
pub mod cpuid_detection;
pub mod topology;
pub mod cache_topology;
pub mod memory_ordering;
pub mod msr_definitions;
pub mod tsc_calibration;
pub mod power_management;
pub mod mtrr_pat;
pub mod errata_workarounds;
pub mod model_comparison;
pub mod acpi_real;
pub mod msr_init;
pub mod fastos_cpu;

// ── Re-exports (consolidated public API) ───────────────────────────
pub use cpuid_detection::{
    detect_cpu, CpuVendor, CpuFamilyModel, CpuIdentity, CpuBrandString,
};
pub use topology::{Topology, CpuId, PerCpu};
pub use tsc_calibration::{calibrate_tsc, TscSource};
pub use mtrr_pat::{init_mtrr, init_pat};
pub use acpi_real::{
    find_rsdp, parse_rsdp, parse_xsdt, parse_mcfg, pm_timer_port,
    RsdpHeader, McfgHeader, AcpiError, LegacyMcfgView,
};
pub use errata_workarounds::{
    apply_spectre_v2_mitigations, apply_spectre_v4_mitigations,
    apply_mds_mitigations, issue_ibpb,
};
pub use msr_init::init_msr_common;
pub use fastos_cpu::{
    init_fastos_cpu, init_msrs, init_acpi,
    identity, topology as fastos_topology, cache, tsc_freq_hz, tsc_source,
    is_initialized, summary,
};

// ── Static information about the Zen 3 profile ─────────────────────
//
// Used by `profile/amd_ryzen_5_5600x.rs` to declare hardware
// capabilities. These are static constants — they don't change at
// runtime.
pub mod info {
    //! Static information about the Zen 3 profile.

    /// CPU family (0x19 for Zen 3).
    pub const FAMILY: u8 = 0x19;
    /// CPU model (0x01 for Vermeer / 5600X).
    pub const MODEL: u8 = 0x01;
    /// Number of physical cores.
    pub const CORES: u8 = 6;
    /// Number of logical threads (cores * SMT).
    pub const THREADS: u8 = 12;
    /// Number of CCXs in the package.
    pub const CCX_COUNT: u8 = 1;
    /// Number of CCDs in the package.
    pub const CCD_COUNT: u8 = 1;
    /// L1 data cache per core, in KB.
    pub const L1D_KB: u32 = 32;
    /// L1 instruction cache per core, in KB.
    pub const L1I_KB: u32 = 32;
    /// L2 cache per core, in KB.
    pub const L2_KB: u32 = 512;
    /// L3 cache per package, in MB.
    pub const L3_MB: u32 = 32;
    /// Base frequency, in Hz.
    pub const BASE_HZ: u64 = 3_700_000_000;
    /// Max boost frequency, in Hz.
    pub const BOOST_HZ: u64 = 4_600_000_000;
    /// Whether AVX-512 is supported (5600X: NO).
    pub const HAS_AVX512: bool = false;
    /// Whether 5-level paging (LA57) is supported (5600X: NO).
    pub const HAS_LA57: bool = false;
    /// Whether the TSC is invariant (5600X: NO — varies with P-state).
    pub const HAS_INVARIANT_TSC: bool = false;
    /// Whether SMT (Hyper-Threading) is enabled.
    pub const HAS_SMT: bool = true;
    /// Microarchitecture name.
    pub const UARCH_NAME: &str = "Zen 3 (Vermeer)";
    /// Codename.
    pub const CODE_NAME: &str = "Vermeer";
    /// Manufacturing process.
    pub const PROCESS: &str = "TSMC 7FF";
}
