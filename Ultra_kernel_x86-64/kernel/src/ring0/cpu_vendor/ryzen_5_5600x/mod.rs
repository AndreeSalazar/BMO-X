//! Ryzen 5 5600X (Vermeer, Zen 3, Family 19h Model 01h) profile.
//!
//! This module is the canonical "CPU profile" for the FastOS test
//! bench. It bundles:
//!
//! - `cpuid` -- vendor/family/model/brand detection (the legacy
//!   `crates_Personal/ring0/cpu_vendor_profile/src/amd/cpu/zen3/cpuid_detection.rs`,
//!   simplified and re-exported here for in-kernel use).
//! - `topology` -- SMT/CCX/CCD layout via CPUID 0x0B / 0x8000001E.
//! - `cache` -- L1d/L1i/L2/L3 sizes via CPUID 0x80000005/06/1D.
//! - `tsc` -- TSC calibration via CPUID 0x15 with ACPI PM Timer fallback.
//! - `errata` -- Spectre v2 / v4 (SSB) / MDS workarounds for Zen 3.
//! - `bmo_cpu` -- consolidated `init_bmo_cpu()` that runs all the
//!   above once and stashes results in static globals.

pub mod cpuid;
pub mod topology;
pub mod cache;
pub mod tsc;
pub mod errata;
pub mod bmo_cpu;
/// **Lo que una puerta tiene permitido costar EN ESTE SILICIO.** Vive aqui y no
/// en `syscall/` porque un techo en ticks es un dato de este CPU: ver su
/// cabecera.
pub mod presupuesto;

pub use bmo_cpu::init_bmo_cpu;

/// Profile descriptor consumed by `cpu_vendor::profile::active()`.
/// Los contadores de energia de este Zen 3. Son de AMD: por eso viven aqui y
/// no en `ring0/cpu/`. Ver su cabecera.
pub mod power;

/// The rest of Ring 0 sees only this -- never this module directly.
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
    // * Antes decia 0b111 y el area 832, que son los numeros de lo HABILITADO,
    // no de lo soportado. El verificador cantaba DIFIERE en cada arranque -- y
    // un aviso ambar que sale siempre deja de ser un aviso: es ruido que
    // ensena a ignorar la linea justo el dia que importe. Los dos campos se
    // contrastan contra cosas distintas y hay que darles los numeros de cada
    // una: `xsave_componentes` contra CPUID.D.0:EDX:EAX, `xsave_area` contra
    // CPUID.D.0:ECX (el area con TODO habilitado), y `xsave_xcr0` contra lo
    // que el firmware dejo puesto.
    xsave_componentes: 0x207,
    // Y los tres HABILITADOS, que aqui coincide con lo soportado -- pero por una
    // razon que no es del CPU sino del firmware: esta placa deja XCR0 = 0x7
    // puesto antes de que BMO arranque. Se declara aparte precisamente porque
    // el dia que coincidan por casualidad y luego dejen de coincidir, esto es
    // lo unico que lo va a ver.
    xsave_xcr0: 0b111,
    // El area con TODO lo soportado habilitado (CPUID.D.0:ECX), no la de los
    // componentes de hoy. Con XCR0 = 0x7 el CPU usa 832; si alguien encendiera
    // PKRU harian falta 2440. Reservamos 1024, que cubre lo primero con holgura
    // -- y `xsave::init` se planta al arrancar si algun dia no cubriera.
    xsave_area: 2440,
    nucleos: nucleos,
    // ** Lo que ESTE chip es, escrito para poder DESMENTIR al silicio. Un
    // Ryzen 5 5600X es 6/12 y no hay ninguno que no lo sea: si CPUID contesta
    // otra cosa, la respuesta esta mal -- y esto es lo unico en todo el arbol
    // que tiene derecho a afirmarlo, porque es lo unico que sabe que chip es.
    topologia_esperada: Some((6, 12)),
    // El LECTOR de energia de este silicio. Un perfil sin RAPL pondria `None` y
    // la terminal lo diria con palabras en vez de pintar 0 W -- ver el campo
    // `energia` de `CpuProfile`, donde esta el porque de que sea `Option`.
    energia: Some(power::leer),
    // Las tres filas de ciclos medidas en ESTA placa. Ver `presupuesto.rs`: si
    // el CPU de delante no es este, cruzan a Ring 3 como "sin declarar" y el
    // juez se calla en vez de inventarse un veredicto.
    presupuesto: &presupuesto::PRESUPUESTO,
    identidad: identidad,
};

/// Lo que el silicio contesta a CPUID, subido al contrato del perfil.
///
/// `None` mientras `init` no haya corrido -- y eso NO es un fallo: significa
/// *"todavia no se ha preguntado"*, que es distinto de *"es otro CPU"*. Quien
/// lo lee tiene que tratarlo como "no se sabe", que es lo que hace
/// `presupuesto::es_esta_maquina` al negarse a juzgar.
fn identidad() -> Option<(u8, u8)> {
    let id = bmo_cpu::identity()?;
    Some((id.family_model.family, id.family_model.model))
}

/// Sube la topologia del Ryzen al contrato neutral del perfil.
///
/// Aqui abajo `Topology` tiene ocho campos y un array de 64 `CpuId`; hacia
/// arriba salen cuatro numeros. **Esa reduccion es el contrato**: Ring 0 no
/// tiene por que saber que es un CCD, solo cuantos hay.
fn nucleos() -> Option<super::profile::Nucleos> {
    let t = bmo_cpu::topology()?;
    Some(super::profile::Nucleos {
        nucleos: t.total_cores,
        hilos: t.total_threads,
        ccx: t.total_ccxs,
        ccd: t.total_ccds,
        // ** Y suben tambien las dos que dicen CUANTO fiarse. Ver `Nucleos`.
        hilos_por_nucleo: t.hilos_por_nucleo,
        discrepan: t.discrepan,
    })
}
