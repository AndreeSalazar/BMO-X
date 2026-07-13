//! CPU feature detection for the Ryzen 5 5600X.

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

    /// Build from a detected CpuIdentity (local shim).
    pub fn from_identity(id: &super::vendor_shim::zen3::cpuid_detection::CpuIdentity) -> Self {
        use super::vendor_shim::zen3::cpuid_detection;
        Self {
            has_sse: true,  // always true on x86-64
            has_sse2: cpuid_detection::has_sse2(id),
            has_avx: cpuid_detection::has_avx(id),
            has_avx2: cpuid_detection::has_avx2(id),
            has_xsave: (id.features_ecx & (1 << 26)) != 0,
            has_osxsave: (id.features_ecx & (1 << 27)) != 0,
            has_fs_gs_base: cpuid_detection::has_fsgsbase(id),
            has_smep: cpuid_detection::has_smep(id),
            has_smap: cpuid_detection::has_smap(id),
            has_umip: (cpuid_detection::cpuid(7, 0).2 & (1 << 2)) != 0,
            has_mtrr: (id.features_edx & (1 << 12)) != 0,
            has_perfctr_core: (id.features_ecx & (1 << 23)) != 0,
        }
    }
}

/// Detect CPU features. Tries the real CPUID path first; falls back to
/// the hardcoded 5600X set if the cpuid call fails.
pub fn detect() -> CpuFeatures {
    if let Some(id) = super::vendor_shim::zen3::cpuid_detection::identity() {
        return CpuFeatures::from_identity(id);
    }
    CpuFeatures::for_5600x()
}
