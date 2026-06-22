//! `vendor/amd/cpu/zen3/` — Knowledge of the AMD Ryzen 5 5600X (Zen 3).
//!
//! v1.8.8: this is a re-export of `crate::AMD::zen3` to migrate toward
//! the new architecture. **No code moved yet** — all logic remains in
//! `kernel/src/ring0/AMD/zen3/` for backwards compatibility. Future
//! versions will move the actual implementations here.
//!
//! ## Future layout (Phase 1+)
//!
//! ```
//! vendor/amd/cpu/zen3/
//! ├── mod.rs
//! ├── cpuid.rs       (was: AMD/zen3/cpuid_detection.rs)
//! ├── topology.rs    (was: AMD/zen3/topology.rs)
//! ├── cache.rs       (was: AMD/zen3/cache_topology.rs)
//! ├── tsc.rs         (was: AMD/zen3/tsc_calibration.rs)
//! ├── msr.rs         (was: AMD/zen3/msr_definitions.rs + msr_init.rs)
//! ├── mtrr_pat.rs    (was: AMD/zen3/mtrr_pat.rs)
//! ├── power.rs       (was: AMD/zen3/power_management.rs)
//! ├── errata.rs      (was: AMD/zen3/errata_workarounds.rs)
//! ├── acpi.rs        (was: AMD/zen3/acpi_real.rs)
//! ├── memory_ordering.rs
//! ├── model_comparison.rs
//! └── fastos_cpu.rs   (consolidated public API)
//! ```
//!
//! ## What this module exports today
//!
//! All public types and functions of the existing AMD/zen3 implementation,
//! accessible via the new path `crate::vendor::amd::cpu::zen3::*`.

#![allow(dead_code)] // v1.8.8: re-export only; original #[allow] in source

// ── Re-export from old location for backwards compatibility ───────
pub use crate::amd_cpu::zen3::*;

// ── Module index (informational) ───────────────────────────────────
//
// These constants describe the Zen 3 microarchitecture features that
// this profile enables. They are used by `profile/amd_ryzen_5_5600x.rs`
// to declare hardware capabilities.
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
