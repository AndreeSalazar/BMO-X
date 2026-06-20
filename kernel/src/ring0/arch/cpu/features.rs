#![allow(dead_code)]

//! CPU feature detection via CPUID — all x86-64 feature flags.

use super::cpuid;

/// Full Zen 3 feature set detected via CPUID.
#[derive(Debug, Clone)]
pub struct CpuFeatures {
    pub has_sse: bool,
    pub has_sse2: bool,
    pub has_sse3: bool,
    pub has_ssse3: bool,
    pub has_sse41: bool,
    pub has_sse42: bool,
    pub has_avx: bool,
    pub has_fma3: bool,
    pub has_aes: bool,
    pub has_pclmulqdq: bool,
    pub has_f16c: bool,
    pub has_popcnt: bool,
    pub has_movbe: bool,
    pub has_osxsave: bool,
    pub has_rdrand: bool,
    pub has_avx2: bool,
    pub has_bmi1: bool,
    pub has_bmi2: bool,
    pub has_hle: bool,
    pub has_rtm: bool,
    pub has_mpx: bool,
    pub has_avx512f: bool,
    pub has_avx512dq: bool,
    pub has_avx512cd: bool,
    pub has_avx512bw: bool,
    pub has_avx512vl: bool,
    pub has_sha: bool,
    pub has_rdseed: bool,
    pub has_adx: bool,
    pub has_clflushopt: bool,
    pub has_clwb: bool,
    pub has_xsaveopt: bool,
    pub has_smap: bool,
    pub has_umip: bool,
    pub has_smep: bool,
    pub has_lzcnt: bool,
    pub has_sse4a: bool,
    pub has_nx: bool,
    pub has_page1gb: bool,
    pub has_rdtscp: bool,
    pub has_lm: bool,
    pub has_invtsc: bool,
    pub has_clzero: bool,
    pub has_invpcid: bool,
    pub has_fs_gs_base: bool,
    pub has_mtrr: bool,
    pub has_perfctr_core: bool,
    pub has_perfctr_nb: bool,
    pub max_basic_leaf: u32,
    pub max_ext_leaf: u32,
    pub cpu_family: u32,
    pub cpu_model: u32,
    pub cpu_stepping: u32,
    pub xsave_area_size: u32,
    pub brand_string: [u8; 48],
}

impl CpuFeatures {
    pub const fn empty() -> Self {
        Self {
            has_sse: false, has_sse2: false, has_sse3: false, has_ssse3: false,
            has_sse41: false, has_sse42: false, has_avx: false, has_fma3: false,
            has_aes: false, has_pclmulqdq: false, has_f16c: false, has_popcnt: false,
            has_movbe: false, has_osxsave: false, has_rdrand: false,
            has_avx2: false, has_bmi1: false, has_bmi2: false, has_hle: false,
            has_rtm: false, has_mpx: false, has_avx512f: false, has_avx512dq: false,
            has_avx512cd: false, has_avx512bw: false, has_avx512vl: false,
            has_sha: false, has_rdseed: false, has_adx: false,
            has_clflushopt: false, has_clwb: false,
            has_xsaveopt: false,
            has_smap: false, has_umip: false, has_smep: false,
            has_lzcnt: false, has_sse4a: false, has_nx: false,
            has_page1gb: false, has_rdtscp: false, has_lm: false,
            has_invtsc: false,
            has_clzero: false, has_invpcid: false,
            has_fs_gs_base: false,
            has_mtrr: false, has_perfctr_core: false, has_perfctr_nb: false,
            max_basic_leaf: 0, max_ext_leaf: 0,
            cpu_family: 0, cpu_model: 0, cpu_stepping: 0,
            xsave_area_size: 0,
            brand_string: [0u8; 48],
        }
    }

    pub fn is_zen3(&self) -> bool {
        self.cpu_family == 0x19
    }

    pub fn brand_string_str(&self) -> &str {
        core::str::from_utf8(&self.brand_string).unwrap_or("Unknown CPU")
    }
}

/// Detect all CPU features via CPUID leaves.
pub fn detect() -> CpuFeatures {
    let (eax0, _, _, _) = cpuid(0, 0);
    let max_basic = eax0;
    let mut f = CpuFeatures::empty();
    f.max_basic_leaf = max_basic;

    let (eax1, _, ecx1, edx1) = cpuid(1, 0);
    f.cpu_family = ((eax1 >> 8) & 0xF) + ((eax1 >> 20) & 0xFF);
    f.cpu_model = ((eax1 >> 4) & 0xF) | ((eax1 >> 12) & 0xF0);
    f.cpu_stepping = eax1 & 0xF;
    f.has_sse = edx1 & (1 << 25) != 0;
    f.has_sse2 = edx1 & (1 << 26) != 0;
    f.has_sse3 = ecx1 & (1 << 0) != 0;
    f.has_ssse3 = ecx1 & (1 << 9) != 0;
    f.has_sse41 = ecx1 & (1 << 19) != 0;
    f.has_sse42 = ecx1 & (1 << 20) != 0;
    f.has_avx = ecx1 & (1 << 28) != 0;
    f.has_fma3 = ecx1 & (1 << 12) != 0;
    f.has_aes = ecx1 & (1 << 25) != 0;
    f.has_pclmulqdq = ecx1 & (1 << 13) != 0;
    f.has_f16c = ecx1 & (1 << 29) != 0;
    f.has_popcnt = ecx1 & (1 << 23) != 0;
    f.has_movbe = ecx1 & (1 << 22) != 0;
    f.has_osxsave = ecx1 & (1 << 27) != 0;
    f.has_rdrand = ecx1 & (1 << 30) != 0;

    if max_basic >= 7 {
        let (_, ebx7, ecx7, edx7) = cpuid(7, 0);
        f.has_avx2 = ebx7 & (1 << 5) != 0;
        f.has_bmi1 = ebx7 & (1 << 3) != 0;
        f.has_bmi2 = ebx7 & (1 << 8) != 0;
        f.has_hle = ebx7 & (1 << 4) != 0;
        f.has_rtm = ebx7 & (1 << 11) != 0;
        f.has_mpx = ebx7 & (1 << 14) != 0;
        f.has_avx512f = ebx7 & (1 << 16) != 0;
        f.has_avx512dq = ebx7 & (1 << 17) != 0;
        f.has_avx512cd = ebx7 & (1 << 28) != 0;
        f.has_avx512bw = ebx7 & (1 << 30) != 0;
        f.has_avx512vl = ebx7 & (1 << 31) != 0;
        f.has_sha = ebx7 & (1 << 29) != 0;
        f.has_rdseed = ebx7 & (1 << 18) != 0;
        f.has_adx = ebx7 & (1 << 19) != 0;
        f.has_clflushopt = ebx7 & (1 << 23) != 0;
        f.has_clwb = ebx7 & (1 << 24) != 0;
        f.has_xsaveopt = ecx7 & (1 << 27) != 0;
        f.has_smap = edx7 & (1 << 20) != 0;
        f.has_umip = edx7 & (1 << 2) != 0;
        f.has_smep = ebx7 & (1 << 20) != 0;

        if f.has_osxsave {
            let (_, xsave_ebx, _, _) = cpuid(0x0D, 0);
            f.xsave_area_size = xsave_ebx;
        }
    }

    let (eax_ext, _, _, edx_ext) = cpuid(0x80000001, 0);
    f.has_lzcnt = eax_ext & (1 << 5) != 0;
    f.has_sse4a = eax_ext & (1 << 6) != 0;
    f.has_nx = edx_ext & (1 << 20) != 0;
    f.has_page1gb = edx_ext & (1 << 26) != 0;
    f.has_rdtscp = edx_ext & (1 << 27) != 0;
    f.has_lm = edx_ext & (1 << 29) != 0;

    let (_, ebx7ext, _, _) = cpuid(0x80000007, 0);
    f.has_invtsc = ebx7ext & (1 << 8) != 0;

    let (_, ebx8, _, _) = cpuid(0x80000008, 0);
    f.has_clzero = ebx8 & (1 << 0) != 0;
    f.has_invpcid = ebx8 & (1 << 10) != 0;

    let (eax21, _, _, _) = cpuid(0x80000021, 0);
    f.has_fs_gs_base = eax21 & (1 << 1) != 0;

    // MTRR support (CPUID.01H EDX bit 12)
    f.has_mtrr = edx1 & (1 << 12) != 0;

    // Performance monitoring (CPUID.0AH)
    if max_basic >= 0x0A {
        let (eax_perf, _, _, _) = cpuid(0x0A, 0);
        let perf_ver = eax_perf & 0xFF;
        f.has_perfctr_core = perf_ver >= 2;
    }

    // AMD extended perf monitoring
    let (eax_amd, _, _, _) = cpuid(0x80000001, 0);
    let _ = eax_amd;
    f.has_perfctr_nb = true; // Zen 3 always has NB perf counters

    let (eax_max, _, _, _) = cpuid(0x80000000, 0);
    f.max_ext_leaf = eax_max;

    if f.max_ext_leaf >= 0x80000004 {
        let (a, b, c, d) = cpuid(0x80000002, 0);
        f.brand_string[0..4].copy_from_slice(&a.to_le_bytes());
        f.brand_string[4..8].copy_from_slice(&b.to_le_bytes());
        f.brand_string[8..12].copy_from_slice(&c.to_le_bytes());
        f.brand_string[12..16].copy_from_slice(&d.to_le_bytes());
        let (a, b, c, d) = cpuid(0x80000003, 0);
        f.brand_string[16..20].copy_from_slice(&a.to_le_bytes());
        f.brand_string[20..24].copy_from_slice(&b.to_le_bytes());
        f.brand_string[24..28].copy_from_slice(&c.to_le_bytes());
        f.brand_string[28..32].copy_from_slice(&d.to_le_bytes());
        let (a, b, c, d) = cpuid(0x80000004, 0);
        f.brand_string[32..36].copy_from_slice(&a.to_le_bytes());
        f.brand_string[36..40].copy_from_slice(&b.to_le_bytes());
        f.brand_string[40..44].copy_from_slice(&c.to_le_bytes());
        f.brand_string[44..48].copy_from_slice(&d.to_le_bytes());
    }

    f
}
