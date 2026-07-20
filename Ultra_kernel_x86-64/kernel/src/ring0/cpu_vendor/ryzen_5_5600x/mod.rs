//! Ryzen 5 5600X (Vermeer, Zen 3, Family 19h Model 01h) profile.
//!
//! This module is the canonical "CPU profile" for the FastOS test
//! bench. It bundles:
//!
//! - `cpuid` — vendor/family/model/brand detection (the legacy
//!   `crates_Personal/ring0/cpu_vendor_profile/src/amd/cpu/zen3/cpuid_detection.rs`,
//!   simplified and re-exported here for in-kernel use).
//! - `topology` — SMT/CCX/CCD layout via CPUID 0x0B / 0x8000001E.
//! - `cache` — L1d/L1i/L2/L3 sizes via CPUID 0x80000005/06/1D.
//! - `tsc` — TSC calibration via CPUID 0x15 with ACPI PM Timer fallback.
//! - `errata` — Spectre v2 / v4 (SSB) / MDS workarounds for Zen 3.
//! - `bmo_cpu` — consolidated `init_bmo_cpu()` that runs all the
//!   above once and stashes results in static globals.

pub mod cpuid;
pub mod topology;
pub mod cache;
pub mod tsc;
pub mod errata;
pub mod bmo_cpu;

pub use bmo_cpu::init_bmo_cpu;

/// Profile descriptor consumed by `cpu_vendor::profile::active()`.
/// The rest of Ring 0 sees only this — never this module directly.
pub static PROFILE: super::profile::CpuProfile = super::profile::CpuProfile {
    vendor: "AMD",
    microarch: "Zen 3 (Vermeer)",
    name: "Ryzen 5 5600X",
    family_model: "19h/21h",
    init: init_bmo_cpu,
};
