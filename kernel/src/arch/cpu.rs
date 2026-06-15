#![allow(dead_code)]

//! CPU feature detection via CPUID — Ryzen 5 5600X (Zen 3).

#[derive(Debug, Clone, Copy)]
pub struct CpuFeatures {
    pub has_sse: bool,
    pub has_sse2: bool,
    pub has_sse3: bool,
    pub has_ssse3: bool,
    pub has_sse41: bool,
    pub has_sse42: bool,
    pub has_avx: bool,
    pub has_avx2: bool,
    pub has_fma3: bool,
    pub has_aes: bool,
    pub has_sha: bool,
    pub has_bmi1: bool,
    pub has_bmi2: bool,
    pub has_rdrand: bool,
    pub has_rdseed: bool,
    pub has_nx: bool,
}

#[inline]
fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        core::arch::asm!(
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

pub fn detect_cpu() -> CpuFeatures {
    let (_, _, ecx1, edx1) = cpuid(1, 0);
    let (_, ebx7, _, _) = cpuid(7, 0);
    let (_, _, _, edx_ext) = cpuid(0x80000001, 0);

    CpuFeatures {
        has_sse:    edx1 & (1 << 25) != 0,
        has_sse2:   edx1 & (1 << 26) != 0,
        has_sse3:   ecx1 & (1 << 0) != 0,
        has_ssse3:  ecx1 & (1 << 9) != 0,
        has_sse41:  ecx1 & (1 << 19) != 0,
        has_sse42:  ecx1 & (1 << 20) != 0,
        has_avx:    ecx1 & (1 << 28) != 0,
        has_fma3:   ecx1 & (1 << 12) != 0,
        has_aes:    ecx1 & (1 << 25) != 0,
        has_rdrand: ecx1 & (1 << 30) != 0,
        has_avx2:   ebx7 & (1 << 5) != 0,
        has_bmi1:   ebx7 & (1 << 3) != 0,
        has_bmi2:   ebx7 & (1 << 8) != 0,
        has_sha:    ebx7 & (1 << 29) != 0,
        has_rdseed: ebx7 & (1 << 18) != 0,
        has_nx:     edx_ext & (1 << 20) != 0,
    }
}

#[inline]
pub fn rdtsc() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe { core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi); }
    ((hi as u64) << 32) | lo as u64
}

#[inline]
pub fn hlt() { unsafe { core::arch::asm!("hlt"); } }

#[inline]
pub fn cli() { unsafe { core::arch::asm!("cli"); } }

#[inline]
pub fn sti() { unsafe { core::arch::asm!("sti"); } }
