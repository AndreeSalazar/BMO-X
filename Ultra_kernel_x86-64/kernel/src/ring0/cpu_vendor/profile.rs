//! Professional CPU-profile contract: swapping the CPU (or the vendor)
//! is a *profile swap*, never a kernel edit.
//!
//! A profile owns everything the kernel must know about one exact CPU:
//! identity, topology/cache init, TSC calibration, MSR setup and errata.
//! The rest of Ring 0 consumes only this descriptor -- it never names a
//! vendor module directly.
//!
//! Adding a CPU:
//!   1. `cpu_vendor/<new_cpu>/` implementing the same surface as
//!      `ryzen_5_5600x` (cpuid, topology, cache, tsc, errata, bmo_cpu).
//!   2. A `PROFILE` descriptor in that module.
//!   3. Point `active()` at it (compile-time; boot-time selection can
//!      layer on later without changing this contract).

/// **Cuantos nucleos hay**, dicho sin nombrar a ningun fabricante.
///
/// * Existe porque el contrato de arriba estaba ROTO en tres sitios: `informe`,
/// el shell y el bring-up de SMP llamaban a
/// `cpu_vendor::ryzen_5_5600x::bmo_cpu::topology()` **por su nombre**, que es
/// exactamente lo que la cabecera de este fichero prohibe. Compilaba, funcionaba
/// y dejaba tres sitios que habria que editar para estrenar otro CPU -- o sea, la
/// definicion de lo que el perfil existe para evitar.
///
/// La estructura del fabricante puede tener veinte campos mas; aqui solo suben
/// los cuatro que Ring 0 necesita para repartir trabajo. Ver `docs/SMP_MAESTRO.md`.
#[derive(Clone, Copy, Debug)]
pub struct Nucleos {
    /// Nucleos FISICOS. Es el numero que manda para repartir computo.
    pub nucleos: u32,
    /// Hilos logicos. Con SMT son el doble, y **no son el doble de potencia**:
    /// dos hilos del mismo nucleo comparten L1, L2 y unidades de ejecucion.
    pub hilos: u32,
    /// Grupos que comparten L3. En un 5600X es **1**; en un Zen 2 son 2, y ahi
    /// hablar entre grupos cuesta un viaje por el Infinity Fabric.
    pub ccx: u32,
    /// Chiplets.
    pub ccd: u32,
}

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

    // -- Lo que este perfil ESPERA del estado extendido (XSAVE) ------------
    //
    // * Esto es una EXPECTATIVA, jamas la fuente de la verdad. El tamano del
    // area de XSAVE y que componentes existen se le preguntan SIEMPRE al
    // silicio (CPUID hoja 0xD); lo de aqui solo sirve para poder AVISAR
    // cuando el CPU que hay delante no es el que el perfil creia.
    //
    // Es la regla de la casa aplicada al procesador: se hardcodean los
    // CONTRATOS, se le preguntan los HECHOS al hardware. Un perfil que
    // dictara el tamano del area seria un kernel que se rompe --en silencio y
    // corrompiendo registros-- el dia que alguien enchufe otro CPU.
    /// Componentes que se espera que el CPU **soporte** (bit 0 x87, 1 SSE,
    /// 2 AVX...). Se contrasta con `CPUID.D.0:EDX:EAX`.
    pub xsave_componentes: u64,
    /// Componentes que se espera que esten **HABILITADOS** en `XCR0`.
    ///
    /// * No es lo mismo que `xsave_componentes`, y confundirlos se paga caro:
    /// *soportado* es lo que el silicio sabe hacer, *habilitado* es lo que
    /// alguien encendio. Soportado superset of habilitado, siempre.
    ///
    /// Este campo existe porque **`XCR0` no lo decide el kernel**. En esta
    /// maquina el firmware ya lo dejo puesto antes de que BMO arrancara (ver
    /// la cabecera de `trap.rs`: AVX venia habilitado de fabrica). O sea que es
    /// un numero que llega de fuera, que cambia el tamano del area de contexto,
    /// y que hasta ahora no vigilaba nadie -- una actualizacion de BIOS podia
    /// moverlo sin que saliera una sola linea.
    ///
    /// Y desde que existe la guardia de cabecera de los epilogos, `XCR0` ademas
    /// **sostiene una comprobacion en el camino mas caliente del kernel**. Un
    /// dato con ese peso no puede ser el unico del perfil que nadie contrasta.
    ///
    /// Sigue siendo EXPECTATIVA, como los otros dos: si el silicio dice otra
    /// cosa, manda el silicio y el verificador lo grita.
    pub xsave_xcr0: u64,
    /// Bytes del area de XSAVE que se esperan para esos componentes.
    pub xsave_area: u32,

    /// Cuantos nucleos e hilos hay, ya detectados por `init`.
    ///
    /// * Es un puntero a funcion y no un numero: los nucleos **se le preguntan
    /// al silicio**, no se declaran. Es la misma regla que gobierna el area de
    /// XSAVE -- se hardcodean los contratos, se preguntan los hechos.
    pub nucleos: fn() -> Option<Nucleos>,
}

/// The compiled-in profile. Today: Ryzen 5 5600X. On another bench this
/// is the single line that changes.
pub fn active() -> &'static CpuProfile {
    &super::ryzen_5_5600x::PROFILE
}
