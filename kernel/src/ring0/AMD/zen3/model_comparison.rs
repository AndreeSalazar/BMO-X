//! Comparison: Zen 3 (5600X) vs Zen 2 vs Zen 4.
//!
//! Implements `AMD/ryzen_5_5600x.md` §16 (Zen 3 vs Zen 2 vs Zen 4).
//!
//! Status: 📋 STUB — informational only. Used to document the differences
//! between CPU generations. Future: use this to detect at boot if the
//! kernel needs different code paths for Zen 2 vs Zen 3 vs Zen 4.
//!
//! References:
//! - Chips and Cheese: "AMD's Zen 3" (Nov 2020)
//! - Chips and Cheese: "AMD's Zen 2" (Mar 2019)
//! - Wikipedia: Zen 2, Zen 3, Zen 4

/// Architecture ID detected from CPUID.0x8000001E.ECX (NodeId).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZenGeneration {
    Zen1,  // Family 17h, Model 0x00-0x0F (Naples, Raven Ridge, Pinnacle Ridge)
    Zen2,  // Family 17h, Model 0x20-0x3F (Rome, Matisse, Renoir)
    Zen3,  // Family 19h, Model 0x00-0x0F (Vermeer, Cezanne) — 5600X
    Zen4,  // Family 19h, Model 0x10-0x1F (Raphael, Genoa)
    Zen5,  // Family 19h, Model 0x20+ (Granite Ridge, future)
    Unknown,
}

impl ZenGeneration {
    /// Identify the generation from the Family/Model encoding.
    pub fn from_family_model(family: u8, model: u8) -> Self {
        match (family, model) {
            (0x17, 0x00..=0x0F) => Self::Zen1,
            (0x17, 0x20..=0x3F) => Self::Zen2,
            (0x19, 0x00..=0x0F) => Self::Zen3,
            (0x19, 0x10..=0x1F) => Self::Zen4,
            (0x19, 0x20..) => Self::Zen5,
            _ => Self::Unknown,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Zen1 => "Zen 1 (2017)",
            Self::Zen2 => "Zen 2 (2019)",
            Self::Zen3 => "Zen 3 (2020)",
            Self::Zen4 => "Zen 4 (2022)",
            Self::Zen5 => "Zen 5 (2024+)",
            Self::Unknown => "Unknown",
        }
    }
}

/// Key differences that matter for kernel code.
#[derive(Debug, Clone, Copy)]
pub struct GenerationDifferences {
    pub generation: ZenGeneration,
    pub l1_assoc: u8,         // 8 on Zen 2/3/4
    pub l2_per_core_kb: u32,  // 512 on Zen 3, 512 on Zen 4
    pub l3_per_ccx_mb: u32,   // 16 on Zen 2, 32 on Zen 3, 32 on Zen 4
    pub has_3d_vcache: bool,  // 3D V-Cache (Zen 3 only, 5800X3D)
    pub has_avx512: bool,     // Zen 4 only (discrete CPUs only)
    pub has_invariant_tsc: bool, // Zen 4 desktop only
    pub smt_threads: u8,      // 2 on Zen 1/2/3/4
}

impl GenerationDifferences {
    /// Returns the differences for Zen 3 (5600X).
    pub const fn zen3_5600x() -> Self {
        Self {
            generation: ZenGeneration::Zen3,
            l1_assoc: 8,
            l2_per_core_kb: 512,
            l3_per_ccx_mb: 32,
            has_3d_vcache: false,
            has_avx512: false,
            has_invariant_tsc: false,
            smt_threads: 2,
        }
    }

    /// Returns the differences for Zen 2.
    pub const fn zen2() -> Self {
        Self {
            generation: ZenGeneration::Zen2,
            l1_assoc: 8,
            l2_per_core_kb: 512,
            l3_per_ccx_mb: 16,
            has_3d_vcache: false,
            has_avx512: false,
            has_invariant_tsc: false,
            smt_threads: 2,
        }
    }

    /// Returns the differences for Zen 4.
    pub const fn zen4() -> Self {
        Self {
            generation: ZenGeneration::Zen4,
            l1_assoc: 8,
            l2_per_core_kb: 1024,
            l3_per_ccx_mb: 32,
            has_3d_vcache: false,
            has_avx512: true,  // desktop Ryzen 7000 has AVX-512
            has_invariant_tsc: true,  // desktop Ryzen 7000 has Invariant TSC
            smt_threads: 2,
        }
    }
}
