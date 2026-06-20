//! Platform CPU detection — Ryzen 5 5600X (Zen 3 / Vermeer) and compatible.

#![allow(dead_code)]

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

/// Full CPU identity detected at boot.
#[derive(Debug, Clone)]
pub struct CpuIdentity {
    pub vendor: Vendor,
    pub brand: [u8; 48],
    pub family: u32,
    pub model: u32,
    pub stepping: u32,
    pub features: FeatureBitmap,
    pub cache: CacheInfo,
    pub virt_addr_bits: u8,
    pub phys_addr_bits: u8,
    pub microarch: Microarch,
}

impl CpuIdentity {
    pub const fn empty() -> Self {
        Self {
            vendor: Vendor::Unknown,
            brand: [0; 48],
            family: 0,
            model: 0,
            stepping: 0,
            features: FeatureBitmap::empty(),
            cache: CacheInfo::empty(),
            virt_addr_bits: 48,
            phys_addr_bits: 40,
            microarch: Microarch::Unknown,
        }
    }

    pub fn brand_str(&self) -> &str {
        let len = self.brand.iter().position(|&b| b == 0).unwrap_or(self.brand.len());
        core::str::from_utf8(&self.brand[..len]).unwrap_or("(invalid)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Unknown,
    Intel,
    AMD,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Microarch {
    Unknown,
    Zen,
    ZenPlus,
    Zen2,
    Zen3,
    Zen4,
    IntelCore,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FeatureBitmap {
    pub fpu: bool, pub vme: bool, pub de: bool, pub pse: bool, pub tsc: bool,
    pub msr: bool, pub pae: bool, pub mce: bool, pub cx8: bool, pub apic: bool,
    pub sep: bool, pub mtrr: bool, pub pge: bool, pub mca: bool, pub cmov: bool,
    pub pat: bool, pub pse36: bool, pub psn: bool, pub clfsh: bool, pub ds: bool,
    pub acpi: bool, pub mmx: bool, pub fxsr: bool, pub sse: bool, pub sse2: bool,
    pub ss: bool, pub htt: bool, pub tm: bool, pub ia64: bool, pub pbe: bool,
    pub sse3: bool, pub pclmulqdq: bool, pub dtes64: bool, pub monitor: bool,
    pub ds_cpl: bool, pub vmx: bool, pub smx: bool, pub est: bool, pub tm2: bool,
    pub ssse3: bool, pub cnxt_id: bool, pub sdbg: bool, pub fma: bool, pub cx16: bool,
    pub xtpr: bool, pub pdcm: bool, pub pcid: bool, pub dca: bool, pub sse4_1: bool,
    pub sse4_2: bool, pub x2apic: bool, pub movbe: bool, pub popcnt: bool,
    pub tsc_deadline: bool, pub aes_ni: bool, pub xsave: bool, pub osxsave: bool,
    pub avx: bool, pub f16c: bool, pub rdrand: bool, pub hypervisor: bool,
    pub fsgsbase: bool, pub bmi1: bool, pub hle: bool, pub avx2: bool, pub smep: bool,
    pub bmi2: bool, pub erms: bool, pub invpcid: bool, pub rtm: bool, pub pqm: bool,
    pub mpx: bool, pub rdseed: bool, pub adx: bool, pub smap: bool, pub clflushopt: bool,
    pub clwb: bool, pub sha_ni: bool, pub avx512f: bool, pub avx512dq: bool,
    pub avx512pf: bool, pub avx512er: bool, pub avx512cd: bool, pub avx512bw: bool,
    pub avx512vl: bool, pub umip: bool, pub pku: bool, pub ospke: bool, pub rdpid: bool,
    pub sgx_lc: bool, pub lahf_lm: bool, pub cmp_legacy: bool, pub svm: bool,
    pub extapic: bool, pub cr8_legacy: bool, pub abm: bool, pub sse4a: bool,
    pub misalignsse: bool, pub prefetchw: bool, pub osvw: bool, pub ibs: bool,
    pub xop: bool, pub skinit: bool, pub wdt: bool, pub lwp: bool, pub fma4: bool,
    pub tbm: bool, pub topoext: bool, pub perfctr_core: bool, pub perfctr_nb: bool,
    pub bpext: bool, pub ptsc: bool, pub ptsc_chk: bool, pub mmxext: bool,
    pub monitorx: bool, pub addr_mask_ext: bool, pub syscall_sysret: bool,
    pub nx: bool, pub pages_1gb: bool, pub rdtscp: bool, pub lm: bool,
    pub amd_3dnow_ext: bool, pub amd_3dnow: bool, pub invtsc: bool,
}

impl FeatureBitmap {
    pub const fn empty() -> Self { Self { ..unsafe { core::mem::zeroed() } } }
    pub fn has_baseline(&self) -> bool { self.sse && self.sse2 && self.sse3 && self.sse4_1 && self.sse4_2 }
    pub fn has_xsave(&self) -> bool { self.xsave && self.osxsave }
    pub fn has_modern_simd(&self) -> bool { self.avx && self.avx2 && self.bmi1 && self.bmi2 }
    pub fn has_aes(&self) -> bool { self.aes_ni }
    pub fn has_sha(&self) -> bool { self.sha_ni }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheInfo {
    pub l1d_size_kb: u32, pub l1d_assoc: u8, pub l1d_line_bytes: u8,
    pub l1i_size_kb: u32, pub l1i_assoc: u8, pub l1i_line_bytes: u8,
    pub l2_size_kb: u32, pub l2_assoc: u8, pub l2_line_bytes: u8,
    pub l3_size_kb: u32, pub l3_assoc: u8, pub l3_line_bytes: u8,
    pub l1_threads_sharing: u8, pub l2_threads_sharing: u8, pub l3_threads_sharing: u8,
}

impl CacheInfo {
    pub const fn empty() -> Self { Self { ..unsafe { core::mem::zeroed() } } }
}

/// Detect the CPU identity by running CPUID leaves in order.
pub fn detect() -> CpuIdentity {
    let mut id = CpuIdentity::empty();

    // 0x00 — max leaf + vendor
    let (max_leaf, v0, v1, v2) = cpuid(0, 0);
    let vendor_bytes: [u8; 12] = [
        v0 as u8, (v0 >> 8) as u8, (v0 >> 16) as u8, (v0 >> 24) as u8,
        v1 as u8, (v1 >> 8) as u8, (v1 >> 16) as u8, (v1 >> 24) as u8,
        v2 as u8, (v2 >> 8) as u8, (v2 >> 16) as u8, (v2 >> 24) as u8,
    ];
    let vendor_str = core::str::from_utf8(&vendor_bytes).unwrap_or("");
    if vendor_str.starts_with("GenuineIntel") {
        id.vendor = Vendor::Intel;
    } else if vendor_str.starts_with("AuthenticAMD") {
        id.vendor = Vendor::AMD;
    } else {
        id.vendor = Vendor::Other;
    }

    // 0x01 — family/model + features
    if max_leaf >= 1 {
        let (eax, _ebx, ecx, edx) = cpuid(1, 0);
        id.stepping = eax & 0xF;
        let base_model = (eax >> 4) & 0xF;
        let base_family = (eax >> 8) & 0xF;
        id.family = if base_family == 0xF { base_family + ((eax >> 20) & 0xFF) } else { base_family };
        id.model = if id.family >= 0x6 { base_model | ((eax >> 12) & 0xF0) } else { base_model };
        decode_features_leaf1(&mut id.features, ecx, edx);
    }

    // 0x07 — extended features
    if max_leaf >= 7 {
        let (_eax, ebx, ecx, _edx) = cpuid(7, 0);
        decode_features_leaf7(&mut id.features, ebx, ecx);
    }

    // 0x8000_0000 — max extended leaf
    let (max_ext, _, _, _) = cpuid(0x8000_0000, 0);
    if max_ext >= 0x8000_0001 {
        let (_, _, ecx, edx) = cpuid(0x8000_0001, 0);
        decode_features_leaf80000001(&mut id.features, ecx, edx);
    }

    // 0x8000_0002-4 — brand string
    if max_ext >= 0x8000_0004 {
        for i in 0..3 {
            let (a, b, c, d) = cpuid(0x8000_0002 + i, 0);
            let off = (i as usize) * 16;
            id.brand[off..off + 4].copy_from_slice(&a.to_le_bytes());
            id.brand[off + 4..off + 8].copy_from_slice(&b.to_le_bytes());
            id.brand[off + 8..off + 12].copy_from_slice(&c.to_le_bytes());
            id.brand[off + 12..off + 16].copy_from_slice(&d.to_le_bytes());
        }
    }

    // 0x8000_0005 — L1 cache
    if max_ext >= 0x8000_0005 {
        let (_eax, _ebx, ecx, edx) = cpuid(0x8000_0005, 0);
        id.cache.l1d_size_kb = (ecx >> 24) & 0xFF;
        id.cache.l1d_assoc = ((ecx >> 16) & 0xFF) as u8;
        id.cache.l1d_line_bytes = (ecx & 0xFF) as u8;
        id.cache.l1i_size_kb = (edx >> 24) & 0xFF;
        id.cache.l1i_assoc = ((edx >> 16) & 0xFF) as u8;
        id.cache.l1i_line_bytes = (edx & 0xFF) as u8;
    }

    // 0x8000_0006 — L2/L3 cache
    if max_ext >= 0x8000_0006 {
        let (_eax, _ebx, ecx, edx) = cpuid(0x8000_0006, 0);
        id.cache.l2_size_kb = (ecx >> 16) & 0xFFFF;
        id.cache.l2_assoc = ((ecx >> 8) & 0xFF) as u8;
        id.cache.l2_line_bytes = (ecx & 0xFF) as u8;
        let l3_units = (edx >> 18) & 0x3FFF;
        id.cache.l3_size_kb = l3_units * 512;
        id.cache.l3_assoc = ((edx >> 8) & 0xFF) as u8;
        id.cache.l3_line_bytes = (edx & 0xFF) as u8;
    }

    // 0x8000_0007 — invariant TSC
    if max_ext >= 0x8000_0007 {
        let (_, _, _, edx) = cpuid(0x8000_0007, 0);
        id.features.invtsc = (edx & (1 << 8)) != 0;
    }

    // 0x8000_0008 — address sizes
    if max_ext >= 0x8000_0008 {
        let (eax, _, _, _) = cpuid(0x8000_0008, 0);
        id.phys_addr_bits = (eax & 0xFF) as u8;
        id.virt_addr_bits = ((eax >> 8) & 0xFF) as u8;
    }

    // Detect microarchitecture
    id.microarch = detect_microarch(id.family, id.model, id.vendor);

    id
}

fn decode_features_leaf1(f: &mut FeatureBitmap, ecx: u32, edx: u32) {
    f.sse3        = (ecx & (1 << 0)) != 0;
    f.pclmulqdq   = (ecx & (1 << 1)) != 0;
    f.dtes64      = (ecx & (1 << 2)) != 0;
    f.monitor     = (ecx & (1 << 3)) != 0;
    f.ds_cpl      = (ecx & (1 << 4)) != 0;
    f.vmx         = (ecx & (1 << 5)) != 0;
    f.smx         = (ecx & (1 << 6)) != 0;
    f.est         = (ecx & (1 << 7)) != 0;
    f.tm2         = (ecx & (1 << 8)) != 0;
    f.ssse3       = (ecx & (1 << 9)) != 0;
    f.fma         = (ecx & (1 << 12)) != 0;
    f.cx16        = (ecx & (1 << 13)) != 0;
    f.pcid        = (ecx & (1 << 17)) != 0;
    f.sse4_1      = (ecx & (1 << 19)) != 0;
    f.sse4_2      = (ecx & (1 << 20)) != 0;
    f.x2apic      = (ecx & (1 << 21)) != 0;
    f.movbe       = (ecx & (1 << 22)) != 0;
    f.popcnt      = (ecx & (1 << 23)) != 0;
    f.tsc_deadline= (ecx & (1 << 24)) != 0;
    f.aes_ni      = (ecx & (1 << 25)) != 0;
    f.xsave       = (ecx & (1 << 26)) != 0;
    f.osxsave     = (ecx & (1 << 27)) != 0;
    f.avx         = (ecx & (1 << 28)) != 0;
    f.f16c        = (ecx & (1 << 29)) != 0;
    f.rdrand      = (ecx & (1 << 30)) != 0;
    f.hypervisor  = (ecx & (1 << 31)) != 0;

    f.fpu         = (edx & (1 << 0)) != 0;
    f.vme         = (edx & (1 << 1)) != 0;
    f.tsc         = (edx & (1 << 4)) != 0;
    f.msr         = (edx & (1 << 5)) != 0;
    f.pae         = (edx & (1 << 6)) != 0;
    f.mce         = (edx & (1 << 7)) != 0;
    f.cx8         = (edx & (1 << 8)) != 0;
    f.apic        = (edx & (1 << 9)) != 0;
    f.sep         = (edx & (1 << 11)) != 0;
    f.mtrr        = (edx & (1 << 12)) != 0;
    f.pge         = (edx & (1 << 13)) != 0;
    f.cmov        = (edx & (1 << 15)) != 0;
    f.pat         = (edx & (1 << 16)) != 0;
    f.pse36       = (edx & (1 << 17)) != 0;
    f.clfsh       = (edx & (1 << 19)) != 0;
    f.acpi        = (edx & (1 << 22)) != 0;
    f.mmx         = (edx & (1 << 23)) != 0;
    f.fxsr        = (edx & (1 << 24)) != 0;
    f.sse         = (edx & (1 << 25)) != 0;
    f.sse2        = (edx & (1 << 26)) != 0;
    f.htt         = (edx & (1 << 28)) != 0;
}

fn decode_features_leaf7(f: &mut FeatureBitmap, ebx: u32, ecx: u32) {
    f.fsgsbase   = (ebx & (1 << 0)) != 0;
    f.bmi1       = (ebx & (1 << 3)) != 0;
    f.avx2       = (ebx & (1 << 5)) != 0;
    f.smep       = (ebx & (1 << 7)) != 0;
    f.bmi2       = (ebx & (1 << 8)) != 0;
    f.erms       = (ebx & (1 << 9)) != 0;
    f.invpcid    = (ebx & (1 << 10)) != 0;
    f.avx512f    = (ebx & (1 << 16)) != 0;
    f.rdseed     = (ebx & (1 << 17)) != 0;
    f.adx        = (ebx & (1 << 19)) != 0;
    f.smap       = (ebx & (1 << 20)) != 0;
    f.clflushopt = (ebx & (1 << 23)) != 0;
    f.clwb       = (ebx & (1 << 24)) != 0;
    f.sha_ni     = (ebx & (1 << 29)) != 0;

    f.umip       = (ecx & (1 << 2)) != 0;
    f.pku        = (ecx & (1 << 3)) != 0;
    f.ospke      = (ecx & (1 << 4)) != 0;
    f.rdpid      = (ecx & (1 << 22)) != 0;
}

fn decode_features_leaf80000001(f: &mut FeatureBitmap, ecx: u32, edx: u32) {
    f.lahf_lm        = (ecx & (1 << 0)) != 0;
    f.svm            = (ecx & (1 << 2)) != 0;
    f.extapic        = (ecx & (1 << 3)) != 0;
    f.cr8_legacy     = (ecx & (1 << 4)) != 0;
    f.abm            = (ecx & (1 << 5)) != 0;
    f.sse4a          = (ecx & (1 << 6)) != 0;
    f.misalignsse    = (ecx & (1 << 7)) != 0;
    f.prefetchw      = (ecx & (1 << 8)) != 0;
    f.osvw           = (ecx & (1 << 9)) != 0;
    f.ibs            = (ecx & (1 << 10)) != 0;
    f.xop            = (ecx & (1 << 11)) != 0;
    f.skinit         = (ecx & (1 << 12)) != 0;
    f.wdt            = (ecx & (1 << 13)) != 0;
    f.fma4           = (ecx & (1 << 16)) != 0;
    f.tbm            = (ecx & (1 << 21)) != 0;
    f.topoext        = (ecx & (1 << 22)) != 0;
    f.perfctr_core   = (ecx & (1 << 23)) != 0;
    f.perfctr_nb     = (ecx & (1 << 24)) != 0;
    f.monitorx       = (ecx & (1 << 29)) != 0;

    f.syscall_sysret = (edx & (1 << 11)) != 0;
    f.nx             = (edx & (1 << 20)) != 0;
    f.pages_1gb      = (edx & (1 << 26)) != 0;
    f.rdtscp         = (edx & (1 << 27)) != 0;
    f.lm             = (edx & (1 << 29)) != 0;
}

fn detect_microarch(family: u32, model: u32, vendor: Vendor) -> Microarch {
    match vendor {
        Vendor::AMD => match family {
            0x17 => match model {
                0x01..=0x07 => Microarch::Zen,
                0x08..=0x1F => Microarch::ZenPlus,
                0x20..=0x3F => Microarch::Zen2,
                _ => Microarch::Unknown,
            },
            0x19 => Microarch::Zen3,
            _ => Microarch::Unknown,
        },
        Vendor::Intel => Microarch::IntelCore,
        _ => Microarch::Unknown,
    }
}
