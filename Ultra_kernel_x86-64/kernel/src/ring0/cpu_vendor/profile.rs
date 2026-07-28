//! Professional CPU-profile contract: swapping the CPU (or the vendor)
//! is a *profile swap*, never a kernel edit.
//!
//! A profile owns everything the kernel must know about one exact CPU:
//! identity, topology/cache init, TSC calibration, MSR setup and errata.
//! The rest of Ring 0 consumes only this descriptor — it never names a
//! vendor module directly.
//!
//! Adding a CPU:
//!   1. `cpu_vendor/<new_cpu>/` implementing the same surface as
//!      `ryzen_5_5600x` (cpuid, topology, cache, tsc, errata, bmo_cpu).
//!   2. A `PROFILE` descriptor in that module.
//!   3. Point `active()` at it (compile-time; boot-time selection can
//!      layer on later without changing this contract).

/// Everything Ring 0 is allowed to know about the CPU it runs on.
pub struct CpuProfile {
    /// Vendor string, e.g. "AMD".
    pub vendor: &'static str,
    /// Microarchitecture, e.g. "Zen 3 (Vermeer)".
    pub microarch: &'static str,
    /// Marketing name, e.g. "Ryzen 5 5600X".
    pub name: &'static str,
    /// CPUID family/model this profile is valid for, e.g. "19h/21h".
    pub family_model: &'static str,
    /// One-shot init: populate identity/topology globals, apply errata
    /// workarounds and speculation mitigations. Called once from
    /// `phase::main` on the BSP.
    pub init: fn(),

    // ── Lo que este perfil ESPERA del estado extendido (XSAVE) ────────────
    //
    // ★ Esto es una EXPECTATIVA, jamás la fuente de la verdad. El tamaño del
    // área de XSAVE y qué componentes existen se le preguntan SIEMPRE al
    // silicio (CPUID hoja 0xD); lo de aquí solo sirve para poder AVISAR
    // cuando el CPU que hay delante no es el que el perfil creía.
    //
    // Es la regla de la casa aplicada al procesador: se hardcodean los
    // CONTRATOS, se le preguntan los HECHOS al hardware. Un perfil que
    // dictara el tamaño del área sería un kernel que se rompe —en silencio y
    // corrompiendo registros— el día que alguien enchufe otro CPU.
    /// Componentes que se espera que el CPU **soporte** (bit 0 x87, 1 SSE,
    /// 2 AVX...). Se contrasta con `CPUID.D.0:EDX:EAX`.
    pub xsave_componentes: u64,
    /// Componentes que se espera que estén **HABILITADOS** en `XCR0`.
    ///
    /// ★ No es lo mismo que `xsave_componentes`, y confundirlos se paga caro:
    /// *soportado* es lo que el silicio sabe hacer, *habilitado* es lo que
    /// alguien encendió. Soportado ⊇ habilitado, siempre.
    ///
    /// Este campo existe porque **`XCR0` no lo decide el kernel**. En esta
    /// máquina el firmware ya lo dejó puesto antes de que BMO arrancara (ver
    /// la cabecera de `trap.rs`: AVX venía habilitado de fábrica). O sea que es
    /// un número que llega de fuera, que cambia el tamaño del área de contexto,
    /// y que hasta ahora no vigilaba nadie — una actualización de BIOS podía
    /// moverlo sin que saliera una sola línea.
    ///
    /// Y desde que existe la guardia de cabecera de los epílogos, `XCR0` además
    /// **sostiene una comprobación en el camino más caliente del kernel**. Un
    /// dato con ese peso no puede ser el único del perfil que nadie contrasta.
    ///
    /// Sigue siendo EXPECTATIVA, como los otros dos: si el silicio dice otra
    /// cosa, manda el silicio y el verificador lo grita.
    pub xsave_xcr0: u64,
    /// Bytes del área de XSAVE que se esperan para esos componentes.
    pub xsave_area: u32,
}

/// The compiled-in profile. Today: Ryzen 5 5600X. On another bench this
/// is the single line that changes.
pub fn active() -> &'static CpuProfile {
    &super::ryzen_5_5600x::PROFILE
}
