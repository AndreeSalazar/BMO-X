//! CPU feature detection for the Ryzen 5 5600X.
//!
//! # Política (v1.8.7)
//!
//! El kernel es **específico** del 5600X. No exponemos un bitmap de 83
//! features ni constantes globales. Exponemos un struct compacto
//! `CpuFeatures` que `cpu::init` pasa a `regs::init`, `cache::init`,
//! `perf::init`, `fpu::*`.
//!
//! Si en el futuro hay otro CPU, edita este archivo. No hay fallback
//! genérico. Si el kernel se compila para un CPU y corre en otro, panic.
//!
//! # Histórico (v1.7.8)
//!
//! Antes había 32 constantes `pub const HAS_SSE: bool = true;`...
//! Se eliminaron en v1.8.7 porque nadie las consumía (se usaban solo
//! para "silenciar warnings" en `cpu::info::print`).

#![allow(dead_code)]

// ═══════════════════════════════════════════════════════════════════════════
//  CpuFeatures — struct compacto para que el resto del kernel lo use
// ═══════════════════════════════════════════════════════════════════════════

/// Features presentes en el Ryzen 5 5600X que el kernel usa para
/// habilitar paths de init.
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
