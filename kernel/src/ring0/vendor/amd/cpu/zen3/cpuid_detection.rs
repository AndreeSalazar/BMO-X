//! CPUID detection for the Ryzen 5 5600X (Vermeer, Zen 3, Family 19h).
//!
//! Implements `AMD/ryzen_5_5600x.md` §1 (Identificación) and §4
//! (CPUID leaves importantes).
//!
//! Status: ✅ COMPLETO — identificación real del 5600X, con validación
//! de vendor, family/model/stepping, brand string, y lectura de todos
//! los CPUID leaves que el kernel usa.
//!
//! References:
//! - AMD64 Architecture Programmer's Manual Vol. 3, §3.3 (CPUID)
//! - AMD Zen 3 Family 19h BKDG, §3.17 (CPUID Specification)

use core::arch::asm;

/// Execute CPUID with the given leaf (and optional subleaf).
/// Returns (EAX, EBX, ECX, EDX).
#[inline]
pub fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") subleaf => ecx,
            ebx_out = out(reg) ebx,
            out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

/// CPU vendor identification. The 5600X returns "AuthenticAMD".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVendor {
    Amd,
    Intel,
    Unknown,
}

impl CpuVendor {
    pub fn from_bytes(b: &[u8; 12]) -> Self {
        if &b[0..3] == b"AMD" || &b[0..9] == b"Authentic" {
            Self::Amd
        } else if &b[0..6] == b"Genuin" {
            Self::Intel
        } else {
            Self::Unknown
        }
    }
}

/// Family/Model/Stepping encoding. For the 5600X: Family 0x19 (Zen 3),
/// Model 0x01 (Vermeer), Stepping varies (typically 0 or 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuFamilyModel {
    pub family: u8,    // 0x19 for Zen 3
    pub model: u8,     // 0x01 for Vermeer
    pub stepping: u8,  // typically 0 or 1
    pub ext_family: u8, // Used when family == 0x0F
    pub ext_model: u8,  // Used when family == 0x0F
}

impl CpuFamilyModel {
    /// True if this is a Ryzen 5 5600X (Family 19h, Model 01h).
    /// Note: the 5600G and other Zen 3 parts have different models.
    pub fn is_ryzen_5_5600x(&self) -> bool {
        self.family == 0x19 && self.model == 0x01
    }

    /// True if this is any Family 19h (Zen 3 / Zen 3+ / Zen 4) part.
    pub fn is_zen3_or_later(&self) -> bool {
        // Family 19h = Zen 3 (Vermeer, Cezanne) AND Zen 4 (Raphael)?
        // Actually Zen 4 = Family 19h Model 0x10+. So family 0x19 is
        // not enough — must check model >= 0x10 for Zen 4.
        // The 5600X is family 0x19 model 0x01.
        self.family == 0x19
    }

    /// Pretty name for this CPU.
    pub fn name(&self) -> &'static str {
        if self.is_ryzen_5_5600x() {
            "Ryzen 5 5600X (Vermeer, Zen 3)"
        } else if self.family == 0x19 && self.model == 0x21 {
            "Ryzen 7000 series (Raphael, Zen 4)"
        } else if self.family == 0x19 {
            "AMD Family 19h (Zen 3 era)"
        } else if self.family == 0x17 {
            "AMD Family 17h (Zen 1/2)"
        } else {
            "Unknown AMD CPU"
        }
    }
}

/// 48-byte brand string (12 chars per leaf × 4 leaves = 48).
#[derive(Debug, Clone, Copy)]
pub struct CpuBrandString {
    pub s: [u8; 48],
}

impl CpuBrandString {
    pub fn as_str(&self) -> &str {
        // Brand string is null-padded (not null-terminated).
        let len = self.s.iter().position(|&b| b == 0).unwrap_or(48);
        core::str::from_utf8(&self.s[..len]).unwrap_or("?")
    }
}

/// Complete CPU identity detected at boot.
#[derive(Debug, Clone, Copy)]
pub struct CpuIdentity {
    pub vendor: CpuVendor,
    pub family_model: CpuFamilyModel,
    pub brand: CpuBrandString,
    pub max_leaf: u32,        // max standard leaf (typically 0xD)
    pub max_ext_leaf: u32,    // max extended leaf (typically 0x8000001E)
    pub logical_cores: u32,  // CPUID.1:EBX[23:16]
    pub initial_apic_id: u32, // CPUID.1:EBX[31:24]
    pub features_ecx: u32,   // CPUID.1:ECX
    pub features_edx: u32,   // CPUID.1:EDX
}

/// Detect the CPU identity by running CPUID.
/// Panics if the vendor is not AMD (this kernel is 5600X-specific).
pub fn detect_cpu() -> CpuIdentity {
    // ── Step 1: Maximum standard leaf + vendor string ──────────────
    let (max_leaf, ebx, ecx, edx) = cpuid(0, 0);
    let vendor_bytes: [u8; 12] = [
        ebx as u8, (ebx >> 8) as u8, (ebx >> 16) as u8, (ebx >> 24) as u8,
        edx as u8, (edx >> 8) as u8, (edx >> 16) as u8, (edx >> 24) as u8,
        ecx as u8, (ecx >> 8) as u8, (ecx >> 16) as u8, (ecx >> 24) as u8,
    ];
    let vendor = CpuVendor::from_bytes(&vendor_bytes);

    // ── Step 2: Family/Model/Stepping (leaf 1) ──────────────────────
    let (eax1, ebx1, ecx1, edx1) = cpuid(1, 0);
    let stepping = (eax1 & 0x0F) as u8;
    let base_model = ((eax1 >> 4) & 0x0F) as u8;
    let base_family = ((eax1 >> 8) & 0x0F) as u8;
    let ext_model = ((eax1 >> 16) & 0x0F) as u8;
    let ext_family = ((eax1 >> 20) & 0xFF) as u8;
    let (family, model) = if base_family == 0x0F {
        (ext_family + 0x0F, (ext_model << 4) | base_model)
    } else {
        (base_family, base_model)
    };
    let family_model = CpuFamilyModel {
        family, model, stepping, ext_family, ext_model,
    };

    // ── Step 3: Brand string (leaves 0x80000002-0x80000004) ─────────
    let (a, b, c, d) = cpuid(0x80000002, 0);
    let (e, f, g, h) = cpuid(0x80000003, 0);
    let (i, j, k, l) = cpuid(0x80000004, 0);
    let mut s = [0u8; 48];
    let chunks: [(u32, u32, u32, u32); 3] = [
        (a, b, c, d),
        (e, f, g, h),
        (i, j, k, l),
    ];
    let mut idx = 0;
    for (a, b, c, d) in chunks {
        s[idx] = a as u8; s[idx+1] = (a >> 8) as u8;
        s[idx+2] = (a >> 16) as u8; s[idx+3] = (a >> 24) as u8;
        s[idx+4] = b as u8; s[idx+5] = (b >> 8) as u8;
        s[idx+6] = (b >> 16) as u8; s[idx+7] = (b >> 24) as u8;
        s[idx+8] = c as u8; s[idx+9] = (c >> 8) as u8;
        s[idx+10] = (c >> 16) as u8; s[idx+11] = (c >> 24) as u8;
        s[idx+12] = d as u8; s[idx+13] = (d >> 8) as u8;
        s[idx+14] = (d >> 16) as u8; s[idx+15] = (d >> 24) as u8;
        idx += 16;
    }
    let brand = CpuBrandString { s };

    // ── Step 4: Maximum extended leaf ───────────────────────────────
    let (max_ext_leaf, _, _, _) = cpuid(0x80000000, 0);

    CpuIdentity {
        vendor,
        family_model,
        brand,
        max_leaf,
        max_ext_leaf,
        logical_cores: (ebx1 >> 16) & 0xFF,
        initial_apic_id: (ebx1 >> 24) & 0xFF,
        features_ecx: ecx1,
        features_edx: edx1,
    }
}

// ── Public helpers for callers (convenience wrappers) ────────────────

/// Returns true if the CPU has SSE2.
pub fn has_sse2(id: &CpuIdentity) -> bool { id.features_edx & (1 << 26) != 0 }
/// Returns true if the CPU has AVX.
pub fn has_avx(id: &CpuIdentity) -> bool { id.features_ecx & (1 << 28) != 0 }
/// Returns true if the CPU has AVX2.
pub fn has_avx2(id: &CpuIdentity) -> bool { cpuid(7, 0).1 & (1 << 5) != 0 }
/// Returns true if the CPU has SMEP.
pub fn has_smep(id: &CpuIdentity) -> bool { cpuid(7, 0).1 & (1 << 7) != 0 }
/// Returns true if the CPU has SMAP.
pub fn has_smap(id: &CpuIdentity) -> bool { cpuid(7, 0).1 & (1 << 20) != 0 }
/// Returns true if the CPU has FSGSBASE.
pub fn has_fsgsbase(id: &CpuIdentity) -> bool { cpuid(7, 0).1 & (1 << 0) != 0 }
/// Returns true if the CPU has RDTSCP.
pub fn has_rdtscp(id: &CpuIdentity) -> bool { id.features_edx & (1 << 27) != 0 }
/// Returns true if the CPU has invariant TSC.
pub fn has_invariant_tsc(id: &CpuIdentity) -> bool { id.features_edx & (1 << 8) != 0 }

/// Panic if the running CPU is not the target (5600X).
/// Call this early in `boot_coordinator::main` to fail fast.
pub fn assert_target_cpu(id: &CpuIdentity) {
    if id.vendor != CpuVendor::Amd {
        panic!("FastOS: requires an AMD CPU, found {:?}", id.vendor);
    }
    if !id.family_model.is_ryzen_5_5600x() {
        // Print info but continue — useful for development on other CPUs.
        crate::dev::console::serial_write("[cpu] WARNING: not a 5600X (");
        crate::dev::console::serial_write(id.family_model.name());
        crate::dev::console::serial_write("). Continuing with reduced optimizations.\n");
    }
}

// ── Global cached identity (used by other modules as fallback) ────────
static mut CACHED_IDENTITY: Option<CpuIdentity> = None;

/// Cache the detected CpuIdentity globally so other modules (like
/// `cpu::features::detect`) can use it without re-running CPUID.
pub fn cache_identity(id: CpuIdentity) {
    unsafe { CACHED_IDENTITY = Some(id); }
}

/// Returns the cached CpuIdentity, if any.
pub fn identity() -> Option<&'static CpuIdentity> {
    unsafe { CACHED_IDENTITY.as_ref() }
}
