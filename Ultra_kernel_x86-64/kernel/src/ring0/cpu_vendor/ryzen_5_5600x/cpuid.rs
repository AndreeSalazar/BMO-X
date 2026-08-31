//! CPUID detection for the Ryzen 5 5600X (Vermeer, Zen 3, Family 19h).
//!
//! [carril]  VERDE     deteccion por CPUID: leer y contestar
//!
//! Recovers the legacy `cpuid_detection.rs` from the deleted
//! `crates_Personal/ring0/cpu_vendor_profile/...` tree, adapted to
//! compile as a sub-module of the minimal Ring 0 kernel (no_std,
//! no allocator, no Format string).
//!
//! Provides:
//! - `cpuid(leaf, sub) -> (eax, ebx, ecx, edx)`
//! - `CpuVendor` (Amd / Intel / Unknown)
//! - `CpuFamilyModel` (family, model, stepping)
//! - `CpuBrandString` (48 bytes, null-padded)
//! - `CpuIdentity` (everything in one struct)
//! - `detect_cpu()` that runs the relevant leaves once

use core::arch::asm;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuFamilyModel {
    pub family: u8,
    pub model: u8,
    pub stepping: u8,
}

impl CpuFamilyModel {
    /// ** `0x21` y no `0x01`: el byte se leyo por fin el 2026-08-17 y esta copia
    /// tambien lo tenia mal. Lo desempato el trinquete del presupuesto, que
    /// compara familia/modelo contra el perfil y se niega a juzgar si no cuadra:
    /// la tanda dijo `[EN PLAZO]`, o sea que cuadro `19h/21h`. Ver el gemelo de
    /// esta funcion en `ring0/cpu/mod.rs`.
    pub fn is_ryzen_5_5600x(&self) -> bool { self.family == 0x19 && self.model == 0x21 }
    pub fn is_zen3(&self) -> bool { self.family == 0x19 }
    pub fn is_zen2(&self) -> bool { self.family == 0x17 }
    pub fn name(&self) -> &'static str {
        if self.is_ryzen_5_5600x() { "Ryzen 5 5600X (Vermeer, Zen 3)" }
        else if self.is_zen3() { "AMD Family 19h (Zen 3 / Zen 4)" }
        else if self.is_zen2() { "AMD Family 17h (Zen 1/2)" }
        else { "Unknown AMD CPU" }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CpuBrandString(pub [u8; 48]);
impl CpuBrandString {
    pub fn as_str(&self) -> &str {
        let len = self.0.iter().position(|&b| b == 0).unwrap_or(48);
        core::str::from_utf8(&self.0[..len]).unwrap_or("?")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CpuIdentity {
    pub vendor: CpuVendor,
    pub family_model: CpuFamilyModel,
    pub brand: CpuBrandString,
    pub max_leaf: u32,
    pub max_ext_leaf: u32,
    pub logical_cores: u32,
    pub initial_apic_id: u32,
    pub features_ecx: u32,
    pub features_edx: u32,
    pub features_ebx7: u32,
}

pub fn detect_cpu() -> CpuIdentity {
    // Vendor + max_leaf
    let (max_leaf, ebx0, ecx0, edx0) = cpuid(0, 0);
    let vendor_bytes: [u8; 12] = [
        ebx0 as u8, (ebx0 >> 8) as u8, (ebx0 >> 16) as u8, (ebx0 >> 24) as u8,
        edx0 as u8, (edx0 >> 8) as u8, (edx0 >> 16) as u8, (edx0 >> 24) as u8,
        ecx0 as u8, (ecx0 >> 8) as u8, (ecx0 >> 16) as u8, (ecx0 >> 24) as u8,
    ];
    let vendor = CpuVendor::from_bytes(&vendor_bytes);

    // Family/Model/Stepping (leaf 1)
    let (eax1, ebx1, ecx1, edx1) = cpuid(1, 0);
    let stepping  = (eax1 & 0x0F) as u8;
    let base_model = ((eax1 >> 4) & 0x0F) as u8;
    let base_family = ((eax1 >> 8) & 0x0F) as u8;
    let ext_model  = ((eax1 >> 16) & 0x0F) as u8;
    let ext_family = ((eax1 >> 20) & 0xFF) as u8;
    let (family, model) = if base_family == 0x0F {
        (ext_family + 0x0F, (ext_model << 4) | base_model)
    } else {
        (base_family, base_model)
    };
    let family_model = CpuFamilyModel { family, model, stepping };

    // Brand string
    let mut s = [0u8; 48];
    let mut idx = 0;
    for &leaf in &[0x80000002u32, 0x80000003, 0x80000004] {
        let (a, b, c, d) = cpuid(leaf, 0);
        for v in [a, b, c, d] {
            if idx < 48 { s[idx] = v as u8; idx += 1; }
            if idx < 48 { s[idx] = (v >> 8) as u8; idx += 1; }
            if idx < 48 { s[idx] = (v >> 16) as u8; idx += 1; }
            if idx < 48 { s[idx] = (v >> 24) as u8; idx += 1; }
        }
    }
    let brand = CpuBrandString(s);

    // Max extended leaf
    let (max_ext_leaf, _, _, _) = cpuid(0x80000000, 0);

    // Features leaf 7 sub-leaf 0 (SMEP, SMAP, FSGSBASE, etc.)
    let (_, ebx7, _, _) = cpuid(7, 0);

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
        features_ebx7: ebx7,
    }
}
