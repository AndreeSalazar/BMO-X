//! Ryzen 5 5600X (Vermeer, Zen 3, Family 19h Model 01h) profile.
//!
//! This module is the canonical "CPU profile" for the FastOS test
//! bench. It bundles:
//!
//! - `cpuid` — vendor/family/model/brand detection (the legacy
//!   `crates_Personal/ring0/cpu_vendor_profile/src/amd/cpu/zen3/cpuid_detection.rs`,
//!   simplified and re-exported here for in-kernel use).
//! - `topology` — SMT/CCX/CCD layout via CPUID 0x0B / 0x8000001E.
//! - `cache` — L1d/L1i/L2/L3 sizes via CPUID 0x80000005/06/1D.
//! - `tsc` — TSC calibration via CPUID 0x15 with ACPI PM Timer fallback.
//! - `errata` — Spectre v2 / v4 (SSB) / MDS workarounds for Zen 3.
//! - `bmo_cpu` — consolidated `init_bmo_cpu()` that runs all the
//!   above once and stashes results in static globals.

pub mod cpuid;
pub mod topology;
pub mod cache;
pub mod tsc;
pub mod errata;
pub mod bmo_cpu;

pub use bmo_cpu::init_bmo_cpu;

/// Profile descriptor consumed by `cpu_vendor::profile::active()`.
/// The rest of Ring 0 sees only this — never this module directly.
pub static PROFILE: super::profile::CpuProfile = super::profile::CpuProfile {
    vendor: "AMD",
    microarch: "Zen 3 (Vermeer)",
    name: "Ryzen 5 5600X",
    family_model: "19h/21h",
    init: init_bmo_cpu,
    // Lo que este CPU SOPORTA, medido con CPUID hoja 0xD en el propio Ryzen el
    // 2026-07-27: x87 (bit 0) + SSE (bit 1) + AVX/YMM alto (bit 2) + PKRU
    // (bit 9) = 0x207. No hay AVX-512 en Vermeer.
    //
    // ★ Antes decia 0b111 y el area 832, que son los numeros de lo HABILITADO,
    // no de lo soportado. El verificador cantaba DIFIERE en cada arranque — y
    // un aviso ambar que sale siempre deja de ser un aviso: es ruido que
    // enseña a ignorar la linea justo el dia que importe. Los dos campos se
    // contrastan contra cosas distintas y hay que darles los numeros de cada
    // una: `xsave_componentes` contra CPUID.D.0:EDX:EAX, `xsave_area` contra
    // CPUID.D.0:ECX (el area con TODO habilitado), y `xsave_xcr0` contra lo
    // que el firmware dejo puesto.
    xsave_componentes: 0x207,
    // Y los tres HABILITADOS, que aqui coincide con lo soportado — pero por una
    // razon que no es del CPU sino del firmware: esta placa deja XCR0 = 0x7
    // puesto antes de que BMO arranque. Se declara aparte precisamente porque
    // el dia que coincidan por casualidad y luego dejen de coincidir, esto es
    // lo unico que lo va a ver.
    xsave_xcr0: 0b111,
    // El area con TODO lo soportado habilitado (CPUID.D.0:ECX), no la de los
    // componentes de hoy. Con XCR0 = 0x7 el CPU usa 832; si alguien encendiera
    // PKRU harian falta 2440. Reservamos 1024, que cubre lo primero con holgura
    // — y `xsave::init` se planta al arrancar si algun dia no cubriera.
    xsave_area: 2440,
    nucleos: nucleos,
};

/// Sube la topología del Ryzen al contrato neutral del perfil.
///
/// Aquí abajo `Topology` tiene ocho campos y un array de 64 `CpuId`; hacia
/// arriba salen cuatro números. **Esa reducción es el contrato**: Ring 0 no
/// tiene por qué saber qué es un CCD, sólo cuántos hay.
fn nucleos() -> Option<super::profile::Nucleos> {
    let t = bmo_cpu::topology()?;
    Some(super::profile::Nucleos {
        nucleos: t.total_cores,
        hilos: t.total_threads,
        ccx: t.total_ccxs,
        ccd: t.total_ccds,
    })
}
