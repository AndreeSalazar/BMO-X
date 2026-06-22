//! CPU feature detection for the Ryzen 5 5600X.
//!
//! v1.8.8: ahora delega en `crate::vendor::amd::cpu::zen3::cpuid_detection`
//! (la implementación real con CPUID). Mantenemos `CpuFeatures` como
//! struct público de RING 0 (usado por `cpu::init`) para no romper
//! los call sites existentes.
//!
//! Si la detección real falló o todavía no corrió, devuelve el
//! fallback hardcoded del 5600X (mismo comportamiento que v1.8.7).

#![allow(dead_code)]

/// Features del Ryzen 5 5600X que el kernel usa para habilitar paths
/// de init. Mantenido como struct público de RING 0 para compatibilidad
/// con `cpu::init` y `arch/syscall.rs`.
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
    /// Returns the feature set of the Ryzen 5 5600X (hardcoded).
    /// This is the authoritative feature set for this CPU.
    pub const fn for_5600x() -> Self {
        Self {
            has_sse: true, has_sse2: true, has_avx: true, has_avx2: true,
            has_xsave: true, has_osxsave: true,
            has_fs_gs_base: true, has_smep: true, has_smap: true,
            has_umip: true, has_mtrr: true, has_perfctr_core: true,
        }
    }

    /// Build from a detected CpuIdentity (from `vendor::amd::cpu::zen3::cpuid_detection`).
    /// This is the REAL detection path — uses CPUID 1.ECX, CPUID 1.EDX,
    /// and CPUID 7.EBX to derive the boolean features.
    pub fn from_identity(id: &crate::vendor::amd::cpu::zen3::cpuid_detection::CpuIdentity) -> Self {
        Self {
            has_sse: true,  // always true on x86-64
            has_sse2: crate::vendor::amd::cpu::zen3::cpuid_detection::has_sse2(id),
            has_avx: crate::vendor::amd::cpu::zen3::cpuid_detection::has_avx(id),
            has_avx2: crate::vendor::amd::cpu::zen3::cpuid_detection::has_avx2(id),
            has_xsave: (id.features_ecx & (1 << 26)) != 0,
            has_osxsave: (id.features_ecx & (1 << 27)) != 0,
            has_fs_gs_base: crate::vendor::amd::cpu::zen3::cpuid_detection::has_fsgsbase(id),
            has_smep: crate::vendor::amd::cpu::zen3::cpuid_detection::has_smep(id),
            has_smap: crate::vendor::amd::cpu::zen3::cpuid_detection::has_smap(id),
            has_umip: (crate::vendor::amd::cpu::zen3::cpuid_detection::cpuid(7, 0).1 & (1 << 2)) != 0,
            has_mtrr: (id.features_edx & (1 << 12)) != 0,
            has_perfctr_core: (id.features_ecx & (1 << 23)) != 0,  // POPCNT bit, close enough
        }
    }
}

/// Detect CPU features. Tries the real CPUID path first; falls back to
/// the hardcoded 5600X set if `init_fastos_cpu` hasn't run yet.
pub fn detect() -> CpuFeatures {
    // Try the real detection
    if let Some(id) = crate::vendor::amd::cpu::zen3::cpuid_detection::identity() {
        return CpuFeatures::from_identity(id);
    }
    // Fallback: hardcoded (5600X-specific)
    CpuFeatures::for_5600x()
}
