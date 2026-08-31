//! El CPU: lo que se le pregunta y lo que se le mide.
//!
//! [carril]  VERDE     lo que se le pregunta al CPU
//!
//! `cpuid` y compania contestan lo que la maquina ES. [`frecuencia`] mide lo que
//! esta HACIENDO, que es otra pregunta y por eso es otro fichero -- ver la
//! seccion 9 de `docs/maestro/AXION_MAESTRO.md`.

/// Cuanto GASTA la maquina ahora. Otra MEDIDA, y la que pone numero a AXION.
pub mod power;
/// A que velocidad va el nucleo AHORA. Una MEDIDA, no un dato.
pub mod frequency;

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

#[derive(Debug, Clone, Copy)]
pub struct CpuFamilyModel {
    pub family: u8,
    pub model: u8,
    pub stepping: u8,
    pub ext_family: u8,
    pub ext_model: u8,
}

impl CpuFamilyModel {
    /// ** ESTE BYTE SE LEYO POR FIN EL 2026-08-17, y estaba mal desde el
    /// principio.
    ///
    /// Decia `model == 0x01`, y ademas la rama de abajo llamaba **"Ryzen 7000
    /// (Raphael, Zen 4)"** al `0x21`. O sea que en la maquina del dueno --un
    /// 5600X-- `info` llevaba meses imprimiendo el nombre de otro procesador.
    ///
    /// Nadie lo vio porque el unico sintoma era ese nombre, y un nombre no
    /// rompe nada. Lo desempato el trinquete del presupuesto: compara
    /// familia/modelo contra lo que declara el perfil (`19h/21h`) y **si no
    /// cuadra se niega a juzgar**. La tanda del 17-08 imprimio
    /// `puerta [EN PLAZO] 839, techo 960` -- o sea que cuadro, o sea que el
    /// silicio dice `19h/21h`.
    ///
    /// [!] La rama de Raphael **se borra en vez de corregirse**: para saber su
    /// modelo hay que leerlo en un Raphael, y aqui no hay ninguno. Cambiar una
    /// afirmacion sin medir por otra sin medir no arregla nada -- deja el mismo
    /// fallo con otra cifra. Un Zen 4 caera en la fila generica de familia 19h,
    /// que es verdad.
    pub fn is_ryzen_5_5600x(&self) -> bool {
        self.family == 0x19 && self.model == 0x21
    }
    pub fn name(&self) -> &'static str {
        if self.is_ryzen_5_5600x() {
            "Ryzen 5 5600X (Vermeer, Zen 3)"
        } else if self.family == 0x19 {
            "AMD Family 19h (Zen 3/Zen 4)"
        } else if self.family == 0x17 {
            "AMD Family 17h (Zen 1/2)"
        } else {
            "Unknown AMD CPU"
        }
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
}

pub fn detect_cpu() -> CpuIdentity {
    let (max_leaf, ebx, ecx, edx) = cpuid(0, 0);
    let vendor_bytes: [u8; 12] = [
        ebx as u8, (ebx >> 8) as u8, (ebx >> 16) as u8, (ebx >> 24) as u8,
        edx as u8, (edx >> 8) as u8, (edx >> 16) as u8, (edx >> 24) as u8,
        ecx as u8, (ecx >> 8) as u8, (ecx >> 16) as u8, (ecx >> 24) as u8,
    ];
    let vendor = CpuVendor::from_bytes(&vendor_bytes);

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
    let family_model = CpuFamilyModel { family, model, stepping, ext_family, ext_model };

    let (a, b, c, d) = cpuid(0x80000002, 0);
    let (e, f, g, h) = cpuid(0x80000003, 0);
    let (i, j, k, l) = cpuid(0x80000004, 0);
    let mut s = [0u8; 48];
    let chunks: [(u32, u32, u32, u32); 3] = [(a, b, c, d), (e, f, g, h), (i, j, k, l)];
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
    let brand = CpuBrandString(s);

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
