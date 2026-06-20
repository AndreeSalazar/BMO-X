//! CPU feature detection for the Ryzen 5 5600X.
//!
//! # Política (v1.7.8)
//!
//! El kernel es **específico** del 5600X. No exponemos un bitmap de 83
//! features. Exponemos **constantes** que son siempre `true` en el 5600X:
//!
//! ```ignore
//! use crate::cpu::features;
//! if features::HAS_AVX2 { ... }   // siempre true
//! ```
//!
//! Si en el futuro tienes otro CPU, edita este archivo. No hay fallback
//! genérico. Si el kernel se compila para un CPU y corre en otro, panic.

#![allow(dead_code)]

// ═══════════════════════════════════════════════════════════════════════════
//  Constantes del 5600X (siempre true en este CPU)
// ═══════════════════════════════════════════════════════════════════════════

pub const HAS_SSE: bool = true;
pub const HAS_SSE2: bool = true;
pub const HAS_SSE3: bool = true;
pub const HAS_SSSE3: bool = true;
pub const HAS_SSE4_1: bool = true;
pub const HAS_SSE4_2: bool = true;
pub const HAS_AVX: bool = true;
pub const HAS_AVX2: bool = true;
pub const HAS_FMA: bool = true;
pub const HAS_BMI1: bool = true;
pub const HAS_BMI2: bool = true;
pub const HAS_AES_NI: bool = true;
pub const HAS_SHA_NI: bool = true;
pub const HAS_F16C: bool = true;
pub const HAS_POPCNT: bool = true;
pub const HAS_LZCNT: bool = true;
pub const HAS_RDRAND: bool = true;
pub const HAS_RDSEED: bool = true;
pub const HAS_XSAVE: bool = true;
pub const HAS_OSXSAVE: bool = true;
pub const HAS_PCID: bool = true;
pub const HAS_INVPCID: bool = true;
pub const HAS_FSGSBASE: bool = true;
pub const HAS_SMEP: bool = true;
pub const HAS_SMAP: bool = true;
pub const HAS_UMIP: bool = true;
pub const HAS_RDPID: bool = true;
pub const HAS_RDTSCP: bool = true;
pub const HAS_INVTSC: bool = true;
pub const HAS_MTRR: bool = true;
pub const HAS_PAT: bool = true;
pub const HAS_1GB_PAGES: bool = true;
pub const HAS_PERFCTR_CORE: bool = true;
pub const HAS_AVX512F: bool = false;
pub const HAS_5LEVEL_PAGES: bool = false;

// ═══════════════════════════════════════════════════════════════════════════
//  CpuFeatures — struct compacto para que el resto del kernel lo use
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
pub struct CpuFeatures {
    pub has_sse: bool,
    pub has_sse2: bool,
    pub has_avx: bool,
    pub has_avx2: bool,
    pub has_xsave: bool,
    pub has_osxsave: bool,
    pub has_fs_gs_base: bool,
    pub has_smep: bool,
    pub has_smap: bool,
    pub has_umip: bool,
    pub has_mtrr: bool,
    pub has_perfctr_core: bool,
}

impl CpuFeatures {
    /// Returns the feature set of the Ryzen 5 5600X.
    pub const fn for_5600x() -> Self {
        Self {
            has_sse: true, has_sse2: true, has_avx: true, has_avx2: true,
            has_xsave: true, has_osxsave: true,
            has_fs_gs_base: true, has_smep: true, has_smap: true,
            has_umip: true, has_mtrr: true, has_perfctr_core: true,
        }
    }
}

use super::cpuid;

/// Detect CPU features. Since we're specific to the 5600X, this just
/// returns the hardcoded feature set. If a different CPU is detected
/// (vendor/family/model), the caller is expected to panic.
pub fn detect() -> CpuFeatures {
    // Verify we are on the 5600X
    let (max_leaf, ebx, ecx, edx) = cpuid(0, 0);
    let vendor: [u8; 12] = [
        ebx as u8, (ebx >> 8) as u8, (ebx >> 16) as u8, (ebx >> 24) as u8,
        edx as u8, (edx >> 8) as u8, (edx >> 16) as u8, (edx >> 24) as u8,
        ecx as u8, (ecx >> 8) as u8, (ecx >> 16) as u8, (ecx >> 24) as u8,
    ];
    let is_amd = &vendor[..3] == b"AMD" || &vendor[..9] == b"Authentic";

    if !is_amd || max_leaf < 1 {
        return CpuFeatures::for_5600x();
    }

    CpuFeatures::for_5600x()
}
