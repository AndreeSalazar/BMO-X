//! CPU profiles that are part of the BMO binary contract.
//!
//! This module is intentionally separate from board discovery. It defines
//! the CPU features generated BMO code may *require*. The boot layer still
//! verifies them through CPUID before a BEF is run.

pub mod x86_64_zen3;

pub use x86_64_zen3::{CpuFeatureSet, CpuProfile, X86_64_ZEN3};

#[cfg(feature = "cpu-epyc-zen3")]
pub const ACTIVE: CpuProfile = CpuProfile::EpycZen3;

#[cfg(all(not(feature = "cpu-epyc-zen3"), feature = "cpu-ryzen-5-5600x"))]
pub const ACTIVE: CpuProfile = CpuProfile::Ryzen5_5600X;

#[cfg(all(
    not(feature = "cpu-epyc-zen3"),
    not(feature = "cpu-ryzen-5-5600x"),
    feature = "cpu-x86-64-zen3"
))]
pub const ACTIVE: CpuProfile = CpuProfile::X86_64Zen3;

#[cfg(not(feature = "cpu-x86-64-zen3"))]
compile_error!("bmo-abi v2 requires an explicit supported CPU profile");

#[cfg(all(feature = "cpu-ryzen-5-5600x", feature = "cpu-epyc-zen3"))]
compile_error!("select exactly one BMO machine profile");
