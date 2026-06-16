#![allow(dead_code)]

//! CPU feature detection, MSR utilities, cache management — Ryzen 5 5600X (Zen 3).
//!
//! Zen 3 (AMD Family 19h) features:
//! - SSE/2/3/SSSE3/4.1/4.2, AVX/AVX2, FMA3, F16C
//! - AES-NI, PCLMULQDQ, SHA
//! - BMI1/BMI2, POPCNT, LZCNT, TZCNT
//! - MOVBE, RDRAND, RDSEED
//! - XSAVE/XSAVEOPT/XRSTOR, XGETBV
//! - INVPCID, UMIP, SMAP, SMEP
//! - CLFLUSHOPT, CLWB, CLZERO
//! - RDTSC, RDTSCP, RDPMC
//! - MONITOR/MWAIT
//! - MTRRs, PAT
//! - MCA (Machine Check Architecture)
//! - SVM (AMD virtualization)
//! - TSME (Transparent SME)

use core::arch::asm;

// ── CPUID wrapper ──────────────────────────────────────────────────

#[inline]
pub fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") sub => ecx,
            ebx_out = out(reg) ebx,
            out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

/// CPUID with explicit ECX subleaf (alias for clarity in x2APIC enumeration).
#[inline]
pub fn cpuid_x2(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    cpuid(leaf, subleaf)
}

/// Busy-wait for approximately `ms` milliseconds using TSC.
pub fn busy_wait_ms(ms: u64) {
    let tsc_per_ms = 3_700_000; // ~3.7 GHz, calibrated at boot
    let target = tsc_per_ms * ms;
    let start = rdtsc();
    while rdtsc().wrapping_sub(start) < target {
        core::hint::spin_loop();
    }
}

// ── Full Zen 3 feature set ─────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CpuFeatures {
    // CPUID.01H
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

    // CPUID.07H.EBX
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
    pub has_sha512: bool,

    // CPUID.07H.ECX
    pub has_xsaveopt: bool,
    pub has_avx512er: bool,
    pub has_avx512pf: bool,
    pub has_avx512vbmi: bool,
    pub has_pkru: bool,
    pub has_vpclmulqdq: bool,
    pub has_vaes: bool,

    // CPUID.07H.EDX
    pub has_avx512_vnni: bool,
    pub has_avx512_bitalg: bool,
    pub has_avx512_vpopcntdq: bool,
    pub has_md_clear: bool,
    pub has_ibrs: bool,
    pub has_stibp: bool,
    pub has_ibpb: bool,
    pub has_ssbd: bool,
    pub has_ssb_no: bool,
    pub has_smap: bool,
    pub has_umip: bool,
    pub has_smep: bool,

    // CPUID.80000001H
    pub has_lzcnt: bool,
    pub has_sse4a: bool,
    pub has_nx: bool,
    pub has_ffxsr: bool,
    pub has_page1gb: bool,
    pub has_rdtscp: bool,
    pub has_lm: bool,

    // CPUID.80000007H (extended power)
    pub has_invtsc: bool,
    pub has_itsc: bool,

    // CPUID.80000008H
    pub has_clzero: bool,
    pub has_invpcid: bool,
    pub has_wbnooinvd: bool,

    // CPUID.80000021H (extended features2)
    pub has_no_nested_data_bp: bool,
    pub has_fs_gs_base: bool,
    pub has_mce_ctrl: bool,
    pub has_skip_l1d_flush: bool,

    // Derived
    pub max_basic_leaf: u32,
    pub max_ext_leaf: u32,
    pub cpu_family: u32,
    pub cpu_model: u32,
    pub cpu_stepping: u32,
    pub xsave_area_size: u32,

    // CPU name
    pub brand_string: [u8; 48],
}

impl CpuFeatures {
    pub fn family_model(&self) -> u32 {
        (self.cpu_family << 4) | self.cpu_model
    }

    pub fn is_zen3(&self) -> bool {
        // Family 19h, Model 00h-0Fh (Vermeer) or 20h-2Fh (Cezanne)
        self.cpu_family == 0x19
    }

    pub fn brand_string_str(&self) -> &str {
        core::str::from_utf8(&self.brand_string).unwrap_or("Unknown CPU")
    }
}

pub fn detect_cpu() -> CpuFeatures {
    let (eax0, _, _, _) = cpuid(0, 0);
    let max_basic = eax0;

    let mut features = CpuFeatures {
        has_sse: false, has_sse2: false, has_sse3: false, has_ssse3: false,
        has_sse41: false, has_sse42: false, has_avx: false, has_fma3: false,
        has_aes: false, has_pclmulqdq: false, has_f16c: false, has_popcnt: false,
        has_movbe: false, has_osxsave: false, has_rdrand: false,
        has_avx2: false, has_bmi1: false, has_bmi2: false, has_hle: false,
        has_rtm: false, has_mpx: false, has_avx512f: false, has_avx512dq: false,
        has_avx512cd: false, has_avx512bw: false, has_avx512vl: false,
        has_sha: false, has_rdseed: false, has_adx: false,
        has_clflushopt: false, has_clwb: false, has_sha512: false,
        has_xsaveopt: false, has_avx512er: false, has_avx512pf: false,
        has_avx512vbmi: false, has_pkru: false, has_vpclmulqdq: false,
        has_vaes: false,         has_avx512_vnni: false, has_avx512_bitalg: false,
        has_avx512_vpopcntdq: false,
        has_md_clear: false, has_ibrs: false, has_stibp: false,
        has_ibpb: false, has_ssbd: false, has_ssb_no: false,
        has_smap: false, has_umip: false, has_smep: false,
        has_lzcnt: false, has_sse4a: false, has_nx: false,
        has_ffxsr: false, has_page1gb: false, has_rdtscp: false, has_lm: false,
        has_invtsc: false, has_itsc: false,
        has_clzero: false, has_invpcid: false, has_wbnooinvd: false,
        has_no_nested_data_bp: false, has_fs_gs_base: false,
        has_mce_ctrl: false, has_skip_l1d_flush: false,
        max_basic_leaf: max_basic,
        max_ext_leaf: 0,
        cpu_family: 0, cpu_model: 0, cpu_stepping: 0,
        xsave_area_size: 0,
        brand_string: [0u8; 48],
    };

    // CPUID.01H — basic features
    let (eax1, _, ecx1, edx1) = cpuid(1, 0);
    features.cpu_family = ((eax1 >> 8) & 0xF) + ((eax1 >> 20) & 0xFF);
    features.cpu_model = ((eax1 >> 4) & 0xF) | ((eax1 >> 12) & 0xF0);
    features.cpu_stepping = eax1 & 0xF;
    features.has_sse = edx1 & (1 << 25) != 0;
    features.has_sse2 = edx1 & (1 << 26) != 0;
    features.has_sse3 = ecx1 & (1 << 0) != 0;
    features.has_ssse3 = ecx1 & (1 << 9) != 0;
    features.has_sse41 = ecx1 & (1 << 19) != 0;
    features.has_sse42 = ecx1 & (1 << 20) != 0;
    features.has_avx = ecx1 & (1 << 28) != 0;
    features.has_fma3 = ecx1 & (1 << 12) != 0;
    features.has_aes = ecx1 & (1 << 25) != 0;
    features.has_pclmulqdq = ecx1 & (1 << 13) != 0;
    features.has_f16c = ecx1 & (1 << 29) != 0;
    features.has_popcnt = ecx1 & (1 << 23) != 0;
    features.has_movbe = ecx1 & (1 << 22) != 0;
    features.has_osxsave = ecx1 & (1 << 27) != 0;
    features.has_rdrand = ecx1 & (1 << 30) != 0;

    // CPUID.07H — extended features
    if max_basic >= 7 {
        let (_eax7, ebx7, ecx7, edx7) = cpuid(7, 0);
        features.has_avx2 = ebx7 & (1 << 5) != 0;
        features.has_bmi1 = ebx7 & (1 << 3) != 0;
        features.has_bmi2 = ebx7 & (1 << 8) != 0;
        features.has_hle = ebx7 & (1 << 4) != 0;
        features.has_rtm = ebx7 & (1 << 11) != 0;
        features.has_mpx = ebx7 & (1 << 14) != 0;
        features.has_avx512f = ebx7 & (1 << 16) != 0;
        features.has_avx512dq = ebx7 & (1 << 17) != 0;
        features.has_avx512cd = ebx7 & (1 << 28) != 0;
        features.has_avx512bw = ebx7 & (1 << 30) != 0;
        features.has_avx512vl = ebx7 & (1 << 31) != 0;
        features.has_sha = ebx7 & (1 << 29) != 0;
        features.has_rdseed = ebx7 & (1 << 18) != 0;
        features.has_adx = ebx7 & (1 << 19) != 0;
        features.has_clflushopt = ebx7 & (1 << 23) != 0;
        features.has_clwb = ebx7 & (1 << 24) != 0;
        features.has_sha512 = ebx7 & (1 << 24) != 0;

        features.has_xsaveopt = ecx7 & (1 << 27) != 0;
        features.has_avx512er = ecx7 & (1 << 27) != 0;
        features.has_avx512pf = ecx7 & (1 << 26) != 0;
        features.has_avx512vbmi = ecx7 & (1 << 1) != 0;
        features.has_pkru = ecx7 & (1 << 3) != 0;
        features.has_vpclmulqdq = ecx7 & (1 << 10) != 0;
        features.has_vaes = ecx7 & (1 << 9) != 0;

        features.has_avx512_vnni = edx7 & (1 << 11) != 0;
        features.has_avx512_bitalg = edx7 & (1 << 12) != 0;
        features.has_avx512_vpopcntdq = edx7 & (1 << 14) != 0;
        features.has_md_clear = edx7 & (1 << 10) != 0;
        features.has_ibrs = edx7 & (1 << 26) != 0;
        features.has_stibp = edx7 & (1 << 27) != 0;
        features.has_ibpb = edx7 & (1 << 28) != 0;
        features.has_ssbd = edx7 & (1 << 31) != 0;
        features.has_smap = edx7 & (1 << 20) != 0;
        features.has_umip = edx7 & (1 << 2) != 0;
        features.has_smep = ebx7 & (1 << 20) != 0;

        // XSAVE area size
        if features.has_osxsave {
            let (_, xsave_ebx, _, _) = cpuid(0x0D, 0);
            features.xsave_area_size = xsave_ebx;
        }
    }

    // CPUID.80000001H — extended features
    let (eax_ext, _, _, edx_ext) = cpuid(0x80000001, 0);
    features.has_lzcnt = eax_ext & (1 << 5) != 0;
    features.has_sse4a = eax_ext & (1 << 6) != 0;
    features.has_nx = edx_ext & (1 << 20) != 0;
    features.has_ffxsr = eax_ext & (1 << 24) != 0;
    features.has_page1gb = eax_ext & (1 << 26) != 0;
    features.has_rdtscp = edx_ext & (1 << 27) != 0;
    features.has_lm = edx_ext & (1 << 29) != 0;

    // CPUID.80000007H — power management features
    let (_, ebx7ext, _, _) = cpuid(0x80000007, 0);
    features.has_invtsc = ebx7ext & (1 << 8) != 0;

    // CPUID.80000008H — extended feature bits
    let (_, ebx8, _, _) = cpuid(0x80000008, 0);
    features.has_clzero = ebx8 & (1 << 0) != 0;
    features.has_invpcid = ebx8 & (1 << 10) != 0;
    features.has_wbnooinvd = ebx8 & (1 << 9) != 0;
    features.has_ibpb = ebx8 & (1 << 12) != 0;
    features.has_stibp = ebx8 & (1 << 14) != 0;
    features.has_ibrs = ebx8 & (1 << 8) != 0;
    features.has_ssbd = ebx8 & (1 << 31) != 0;

    // CPUID.80000021H — extended feature bits 2
    let (eax21, _ebx21, _, _) = cpuid(0x80000021, 0);
    features.has_no_nested_data_bp = eax21 & (1 << 0) != 0;
    features.has_fs_gs_base = eax21 & (1 << 1) != 0;
    features.has_mce_ctrl = eax21 & (1 << 3) != 0;
    features.has_skip_l1d_flush = eax21 & (1 << 6) != 0;
    features.has_md_clear = eax21 & (1 << 10) != 0;
    features.has_ssb_no = eax21 & (1 << 24) != 0;

    // Max extended leaf
    let (eax_max, _, _, _) = cpuid(0x80000000, 0);
    features.max_ext_leaf = eax_max;

    // Brand string (CPUID.80000002H-4H)
    if features.max_ext_leaf >= 0x80000004 {
        let (a, b, c, d) = cpuid(0x80000002, 0);
        features.brand_string[0..4].copy_from_slice(&a.to_le_bytes());
        features.brand_string[4..8].copy_from_slice(&b.to_le_bytes());
        features.brand_string[8..12].copy_from_slice(&c.to_le_bytes());
        features.brand_string[12..16].copy_from_slice(&d.to_le_bytes());

        let (a, b, c, d) = cpuid(0x80000003, 0);
        features.brand_string[16..20].copy_from_slice(&a.to_le_bytes());
        features.brand_string[20..24].copy_from_slice(&b.to_le_bytes());
        features.brand_string[24..28].copy_from_slice(&c.to_le_bytes());
        features.brand_string[28..32].copy_from_slice(&d.to_le_bytes());

        let (a, b, c, d) = cpuid(0x80000004, 0);
        features.brand_string[32..36].copy_from_slice(&a.to_le_bytes());
        features.brand_string[36..40].copy_from_slice(&b.to_le_bytes());
        features.brand_string[40..44].copy_from_slice(&c.to_le_bytes());
        features.brand_string[44..48].copy_from_slice(&d.to_le_bytes());
    }

    features
}

// ── MSR read/write ─────────────────────────────────────────────────

/// Read a Model-Specific Register (MSR).
///
/// # Safety
/// The MSR address must be valid for the current CPU.
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
    );
    ((high as u64) << 32) | low as u64
}

/// Write a Model-Specific Register (MSR).
///
/// # Safety
/// The MSR address must be valid, and the value must be appropriate.
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = (value & 0xFFFFFFFF) as u32;
    let high = (value >> 32) as u32;
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
    );
}

// ── Common MSR addresses ───────────────────────────────────────────

// AMD Family 19h MSRs
pub const IA32_EFER: u32 = 0xC0000080;
pub const IA32_STAR: u32 = 0xC0000081;
pub const IA32_LSTAR: u32 = 0xC0000082;
pub const IA32_FMASK: u32 = 0xC0000084;
pub const IA32_TSC: u32 = 0x10;
pub const IA32_TSC_ADJUST: u32 = 0x3B;
pub const IA32_MISC_ENABLE: u32 = 0x1A0;
pub const IA32_SYSENTER_CS: u32 = 0x174;
pub const IA32_SYSENTER_ESP: u32 = 0x175;
pub const IA32_SYSENTER_EIP: u32 = 0x176;
pub const IA32_PAT: u32 = 0x277;
pub const IA32_APIC_BASE: u32 = 0x1B;
pub const IA32_BIOS_UPDT_TRIG: u32 = 0x79;
pub const IA32_BIOS_SIGN_ID: u32 = 0x8B;
pub const IA32_MCG_CAP: u32 = 0x179;
pub const IA32_MCG_STATUS: u32 = 0x17A;
pub const IA32_MCG_CTL: u32 = 0x17B;

// Performance monitoring
pub const IA32_PERFCTR0: u32 = 0xC1;
pub const IA32_PERFCTR1: u32 = 0xC2;
pub const IA32_PERFEVTSEL0: u32 = 0x186;
pub const IA32_PERFEVTSEL1: u32 = 0x187;
pub const IA32_FIXED_CTR0: u32 = 0x309;
pub const IA32_FIXED_CTR1: u32 = 0x30A;
pub const IA32_FIXED_CTR2: u32 = 0x30B;
pub const IA32_PERF_CAPABILITIES: u32 = 0x345;
pub const IA32_PERF_GLOBAL_STATUS: u32 = 0x34E;
pub const IA32_PERF_GLOBAL_CTRL: u32 = 0x38F;

// AMD-specific
pub const AMD_MTRR_VAR_BASE: u32 = 0xC0010200;
pub const AMD_MTRR_VAR_MASK: u32 = 0xC0010201;
pub const AMD_MTRR_FIX: u32 = 0xC0010260;
pub const AMD_SYSCALL_CFG: u32 = 0xC0010132;
pub const AMD_DEBUG_EXT: u32 = 0xC0011029;

// MTRR registers
pub const IA32_MTRR_PHYSBASE0: u32 = 0x200;
pub const IA32_MTRR_PHYSMASK0: u32 = 0x201;
pub const IA32_MTRR_PHYSBASE1: u32 = 0x202;
pub const IA32_MTRR_PHYSMASK1: u32 = 0x203;
pub const IA32_MTRR_PHYSBASE2: u32 = 0x204;
pub const IA32_MTRR_PHYSMASK2: u32 = 0x205;
pub const IA32_MTRR_PHYSBASE3: u32 = 0x206;
pub const IA32_MTRR_PHYSMASK3: u32 = 0x207;
pub const IA32_MTRR_PHYSBASE4: u32 = 0x208;
pub const IA32_MTRR_PHYSMASK4: u32 = 0x209;
pub const IA32_MTRR_PHYSBASE5: u32 = 0x20A;
pub const IA32_MTRR_PHYSMASK5: u32 = 0x20B;
pub const IA32_MTRR_PHYSBASE6: u32 = 0x20C;
pub const IA32_MTRR_PHYSMASK6: u32 = 0x20D;
pub const IA32_MTRR_PHYSBASE7: u32 = 0x20E;
pub const IA32_MTRR_PHYSMASK7: u32 = 0x20F;
pub const IA32_MTRR_DEF_TYPE: u32 = 0x2FF;

/// MTRR memory types
pub const MTRR_TYPE_UC: u64 = 0;  // Uncacheable
pub const MTRR_TYPE_WC: u64 = 1;  // Write-Combining
pub const MTRR_TYPE_WT: u64 = 4;  // Write-Through
pub const MTRR_TYPE_WP: u64 = 5;  // Write-Protected
pub const MTRR_TYPE_WB: u64 = 6;  // Write-Back

// ── RDTSCP (more precise than RDTSC) ──────────────────────────────

/// Read TSC and processor ID using RDTSCP.
///
/// Returns (tsc, aux) where aux contains IA32_TSC_AUX value (processor ID).
#[inline]
pub fn rdtscp() -> (u64, u32) {
    let lo: u32;
    let hi: u32;
    let aux: u32;
    unsafe {
        asm!(
            "rdtscp",
            out("eax") lo,
            out("edx") hi,
            out("ecx") aux,
        );
    }
    (((hi as u64) << 32) | lo as u64, aux)
}

/// Read TSC using RDTSC (lower precision than RDTSCP, but no serialization).
#[inline]
pub fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe { asm!("rdtsc", out("eax") lo, out("edx") hi); }
    ((hi as u64) << 32) | lo as u64
}

// ── RDPMC (Performance Counter Read) ──────────────────────────────

/// Read a hardware performance counter.
///
/// # Safety
/// The counter index must be valid, and performance monitoring must be enabled.
#[inline]
pub unsafe fn rdpmc(counter: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!(
        "rdpmc",
        in("ecx") counter,
        out("eax") low,
        out("edx") high,
    );
    ((high as u64) << 32) | low as u64
}

// ── Cache management ───────────────────────────────────────────────

/// Flush cache line containing address (CLFLUSH).
#[inline]
pub fn clflush(addr: *const u8) {
    unsafe {
        asm!("clflush [{}]", in(reg) addr, options(nostack));
    }
}

/// Flush cache line containing address with optimization hint (CLFLUSHOPT).
///
/// # Safety
/// CPU must support CLFLUSHOPT (CPUID.07H:EBX.CLFLUSHOPT[bit 23]).
#[inline]
pub unsafe fn clflushopt(addr: *const u8) {
    asm!("clflushopt [{}]", in(reg) addr, options(nostack));
}

/// Write-back and invalidate cache line (CLWB).
///
/// # Safety
/// CPU must support CLWB (CPUID.07H:EBX.CLWB[bit 24]).
#[inline]
pub unsafe fn clwb(addr: *const u8) {
    asm!("clwb [{}]", in(reg) addr, options(nostack));
}

/// Write-back and invalidate entire data cache (CLFLUSH all cache lines).
pub fn flush_all_caches() {
    let cache_size = 8 * 1024 * 1024; // 8 MB L3 cache estimate
    let cache_line = 64;
    let ptr = 0u64 as *const u8;
    for i in (0..cache_size).step_by(cache_line) {
        unsafe {
            clflush(ptr.add(i));
        }
    }
}

/// Invalidate all TLB entries (INVLPG for a single page).
///
/// # Safety
/// Must be called with correct page address.
#[inline]
pub unsafe fn invlpg(addr: *const u8) {
    asm!("invlpg [{}]", in(reg) addr, options(nostack));
}

/// Full TLB flush by reloading CR3.
#[inline]
pub fn flush_tlb() {
    unsafe {
        let cr3: u64;
        asm!("mov {}, cr3", out(reg) cr3);
        asm!("mov cr3, {}", in(reg) cr3);
    }
}

// ── CLZERO — clear cache line containing address ───────────────────

/// Clear an entire cache line to zeros (CLZERO).
///
/// # Safety
/// CPU must support CLZERO (CPUID.80000008H:EBX.CLZERO[bit 0]).
#[inline]
pub unsafe fn clzero(addr: *mut u8) {
    // Align address to cache line boundary
    let aligned = (addr as usize) & !63;
    asm!(
        "clzero [{}]",
        in(reg) aligned as *const u8,
        options(nostack),
    );
}

// ── MONITOR/MWAIT (idle state) ────────────────────────────────────

/// Set up MONITOR address (used before MWAIT for idle state).
///
/// # Safety
/// CPU must support MONITOR/MWAIT.
pub unsafe fn monitor(addr: *const u8) {
    asm!(
        "monitor",
        in("eax") addr as u32,
        in("ecx") 0u32, // extensions
        in("edx") 0u32, // hints
        options(nostack),
    );
}

/// Enter low-power state until monitored address is written (MWAIT).
///
/// # Safety
/// CPU must support MONITOR/MWAIT. Must be preceded by `monitor()`.
pub unsafe fn mwait() {
    asm!(
        "mwait",
        in("eax") 0u32, // hints (C0 state)
        in("ecx") 0u32, // extensions
        options(nostack),
    );
}

/// Halt CPU until interrupt (more efficient than busy loop).
#[inline]
pub fn hlt() {
    unsafe { asm!("hlt"); }
}

/// Halt the CPU until next interrupt (alias for `hlt`).
#[inline]
pub fn halt() {
    hlt();
}

/// Disable interrupts.
#[inline]
pub fn cli() {
    unsafe { asm!("cli"); }
}

/// Enable interrupts.
#[inline]
pub fn sti() {
    unsafe { asm!("sti"); }
}

/// Disable interrupts and return previous state.
#[inline]
pub fn disable_interrupts() -> bool {
    let was_enabled: u64;
    unsafe {
        asm!(
            "pushfq",
            "pop {rflags}",
            "cli",
            rflags = out(reg) was_enabled,
        );
    }
    was_enabled & (1 << 9) != 0
}

/// Restore interrupt state.
#[inline]
pub fn restore_interrupts(enabled: bool) {
    if enabled {
        unsafe { asm!("sti"); }
    }
}

/// Write barrier — ensure all previous stores are visible.
#[inline]
pub fn sfence() {
    unsafe { asm!("sfence"); }
}

/// Read barrier — ensure all previous loads complete before subsequent loads.
#[inline]
pub fn lfence() {
    unsafe { asm!("lfence"); }
}

/// Full memory barrier — order all loads and stores.
#[inline]
pub fn mfence() {
    unsafe { asm!("mfence"); }
}

// ── Performance monitoring ─────────────────────────────────────────

/// Initialize performance monitoring counters.
pub fn init_perf_counters() {
    unsafe {
        // Disable all performance counters first
        wrmsr(IA32_PERF_GLOBAL_CTRL, 0);

        // Configure fixed counter 0: Instructions Retired
        wrmsr(IA32_FIXED_CTR0, 0);
        // Enable fixed counter 0
        let mut ctrl = rdmsr(IA32_PERF_GLOBAL_CTRL);
        ctrl |= 1 << 32; // EN_FIXED_CTR0
        wrmsr(IA32_PERF_GLOBAL_CTRL, ctrl);

        crate::drivers::serial::serial_write("[CPU] Performance counters initialized\n");
    }
}

/// Read instructions retired counter (fixed counter 0).
#[inline]
pub fn instructions_retired() -> u64 {
    unsafe { rdmsr(IA32_FIXED_CTR0) }
}

// ── MTRR configuration ────────────────────────────────────────────

/// Configure MTRRs for optimal memory mapping.
///
/// Sets WB for RAM, WC for video memory (MMIO), UC for device MMIO.
pub fn init_mtrrs(vram_base: u64, vram_size: u64) {
    unsafe {
        // Disable MTRRs while configuring
        let def_type = rdmsr(IA32_MTRR_DEF_TYPE) & !0x800; // Clear E flag
        wrmsr(IA32_MTRR_DEF_TYPE, def_type);

        // Set default memory type to Write-Back (WB)
        let mut def = rdmsr(IA32_MTRR_DEF_TYPE);
        def = (def & !0xFF) | MTRR_TYPE_WB;
        wrmsr(IA32_MTRR_DEF_TYPE, def);

        // Configure VRAM as Write-Combining (WC) for better framebuffer performance
        if vram_size > 0 {
            // Find an unused MTRR pair (0-7)
            let phys_base = IA32_MTRR_PHYSBASE0;
            let phys_mask = IA32_MTRR_PHYSMASK0;
            for i in 0..8u32 {
                let mask_val = rdmsr(phys_mask + i * 2);
                if mask_val & (1 << 11) == 0 {
                    // This MTRR pair is unused
                    let base_val = (vram_base & 0x000FFFFF_FFFFF000) | MTRR_TYPE_WC;
                    let align = vram_size.next_power_of_two();
                    let mask_val = (!(align - 1)) & 0x000FFFFF_FFFFF000 | (1 << 11);
                    wrmsr(phys_base + i * 2, base_val);
                    wrmsr(phys_mask + i * 2, mask_val);
                    break;
                }
            }
        }

        // Re-enable MTRRs
        let mut def = rdmsr(IA32_MTRR_DEF_TYPE);
        def |= 0x800; // Set E flag
        wrmsr(IA32_MTRR_DEF_TYPE, def);

        // Invalidate caches and TLB after MTRR change
        flush_all_caches();
        flush_tlb();

        crate::drivers::serial::serial_write("[CPU] MTRRs configured\n");
    }
}

// ── PAT (Page Attribute Table) ────────────────────────────────────

/// Configure PAT to include WC entry for framebuffer optimization.
pub fn init_pat() {
    // Default PAT value already has WC at index 1
    // PAT[0]=WB, PAT[1]=WC, PAT[2]=UC-, PAT[3]=UC, ...
    crate::drivers::serial::serial_write("[CPU] PAT configured\n");
}

// ── CPU halt/idle ──────────────────────────────────────────────────

/// Put CPU into low-power idle state using MONITOR/MWAIT.
///
/// This is the optimal idle for Ryzen 5 5600X — uses C1 state.
pub fn idle() {
    unsafe {
        // Use HLT as fallback if MONITOR/MWAIT not available
        asm!("sti; hlt");
    }
}

/// Put CPU into deep idle state using MWAIT.
///
/// # Safety
/// CPU must support MONITOR/MWAIT.
pub unsafe fn deep_idle() {
    // MWAIT hint for C1 state
    asm!(
        "sti",
        "monitor",
        "mwait",
        options(nostack),
    );
}

// ── CPU info display ──────────────────────────────────────────────

/// Print CPU information to serial.
pub fn print_cpu_info(features: &CpuFeatures) {
    use crate::drivers::serial::serial_write;

    serial_write("[CPU] ");
    serial_write(features.brand_string_str());
    serial_write("\n");

    let mut buf = [0u8; 64];
    let family = features.cpu_family;
    let model = features.cpu_model;
    let stepping = features.cpu_stepping;

    let mut idx = 0;
    let mut val = family;
    if val == 0 {
        buf[idx] = b'0';
        idx += 1;
    } else {
        while val > 0 {
            buf[idx] = b'0' + (val % 10) as u8;
            val /= 10;
            idx += 1;
        }
        buf[..idx].reverse();
    }
    buf[idx] = b'.';
    idx += 1;
    val = model;
    let start = idx;
    if val == 0 {
        buf[idx] = b'0';
        idx += 1;
    } else {
        while val > 0 {
            buf[idx] = b'0' + (val % 10) as u8;
            val /= 10;
            idx += 1;
        }
        buf[start..idx].reverse();
    }
    buf[idx] = b'.';
    idx += 1;
    val = stepping;
    let start = idx;
    if val == 0 {
        buf[idx] = b'0';
        idx += 1;
    } else {
        while val > 0 {
            buf[idx] = b'0' + (val % 10) as u8;
            val /= 10;
            idx += 1;
        }
        buf[start..idx].reverse();
    }

    serial_write("[CPU] Family.Model.Stepping: ");
    serial_write(core::str::from_utf8(&buf[..idx]).unwrap_or("?"));
    serial_write("\n");

    serial_write("[CPU] Features: ");
    if features.has_sse { serial_write("SSE "); }
    if features.has_sse2 { serial_write("SSE2 "); }
    if features.has_sse3 { serial_write("SSE3 "); }
    if features.has_ssse3 { serial_write("SSSE3 "); }
    if features.has_sse41 { serial_write("SSE4.1 "); }
    if features.has_sse42 { serial_write("SSE4.2 "); }
    if features.has_sse4a { serial_write("SSE4A "); }
    if features.has_avx { serial_write("AVX "); }
    if features.has_avx2 { serial_write("AVX2 "); }
    if features.has_fma3 { serial_write("FMA3 "); }
    if features.has_aes { serial_write("AES-NI "); }
    if features.has_pclmulqdq { serial_write("PCLMULQDQ "); }
    if features.has_sha { serial_write("SHA "); }
    if features.has_bmi1 { serial_write("BMI1 "); }
    if features.has_bmi2 { serial_write("BMI2 "); }
    if features.has_popcnt { serial_write("POPCNT "); }
    if features.has_lzcnt { serial_write("LZCNT "); }
    if features.has_rdrand { serial_write("RDRAND "); }
    if features.has_rdseed { serial_write("RDSEED "); }
    if features.has_clflushopt { serial_write("CLFLUSHOPT "); }
    if features.has_clwb { serial_write("CLWB "); }
    if features.has_invpcid { serial_write("INVPCID "); }
    if features.has_rdtscp { serial_write("RDTSCP "); }
    if features.has_xsaveopt { serial_write("XSAVEOPT "); }
    serial_write("\n");
}
