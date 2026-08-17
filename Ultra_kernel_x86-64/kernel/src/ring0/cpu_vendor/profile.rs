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
/// los cuatro que Ring 0 necesita para repartir trabajo. Ver `docs/maestro/SMP_MAESTRO.md`.
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

    // -- ** LO QUE ESTE PERFIL SABE MEDIR DE SI MISMO (2026-08-12) ----------
    //
    // === El fallo de capas que esto corrige ===
    //
    // La primera version de la terminal del CPU (`ring0/cpu/energia.rs`) hacia
    // `use crate::ring0::cpu_vendor::ryzen_5_5600x::power`. O sea que **el
    // codigo comun llamaba al fabricante por su nombre**: en una maquina con
    // otro perfil habria seguido leyendo MSR de AMD, y leer un MSR que no
    // existe es un `#GP` -- un fault de kernel desde un panel que se repinta.
    //
    // El dueno lo pidio por su nombre: *"organiza bien Perfil, luego los que
    // quiere leer, luego el terminal lee lo que el perfil esta reflejando"*.
    //
    // Asi que la cadena es esta y **no se puede saltar ningun eslabon**:
    //
    // ```text
    //    PERFIL          declara QUE se puede medir en este silicio
    //      v
    //    LECTOR          el modulo del fabricante, que sabe DONDE
    //      v
    //    ARITMETICA      `ring0/cpu/*`, que no sabe de fabricantes
    //      v
    //    TERMINAL        `INFO` -> Ring 3, que solo pinta
    // ```
    //
    // === Por que `Option` y no un puntero a secas ===
    //
    // Porque *"este CPU no lo expone"* es una respuesta legitima y frecuente, y
    // tiene que poder decirse **sin inventar una funcion que devuelva ceros**.
    // Un lector falso que contesta 0 W es indistinguible de una maquina que no
    // gasta -- y eso, encendida, no puede ser verdad.
    /// Los contadores de energia (RAPL o equivalente). `None` = este silicio no
    /// los expone, y entonces la terminal lo DICE en vez de pintar cero.
    pub energia: Option<fn() -> Option<EnergiaCruda>>,

    // -- ** LO QUE ESTE PERFIL DICE QUE UNA PUERTA PUEDE COSTAR (2026-08-17) --
    //
    // El presupuesto de ciclos vivia en `syscall/presupuesto.rs` como `const`
    // del kernel, y **el kernel arranca en cualquier x86-64**. `techo: 960` son
    // ticks del TSC de UNA placa: en otro CPU ese numero no es estricto ni
    // laxo, es de otra maquina -- y juzgar con el da una falsa regresion o un
    // falso aprobado. El mismo fallo de siempre: opinar donde no hay derecho.
    //
    // Encaja aqui sin torcer la doctrina de este fichero, y por la misma razon
    // que el area de XSAVE: **es una EXPECTATIVA declarada, y el hecho se le
    // pregunta al silicio** -- corriendo `sys/precio.bex` en la maquina.
    /// Los techos y las metas medidos en ESTE silicio, con la identidad de la
    /// maquina donde se midieron pegada a ellos.
    pub presupuesto: &'static crate::ring0::syscall::presupuesto::Presupuestos,

    /// **Lo que el silicio dice ser**: `(familia, modelo)` de CPUID, ya
    /// detectados por `init`. `None` si todavia no se ha preguntado.
    ///
    /// * Es un puntero a funcion por el mismo motivo que `nucleos`: la
    /// identidad **se le pregunta al CPU**, no se declara. Y existe porque sin
    /// el, comprobar que el presupuesto es de esta maquina obligaria a nombrar
    /// el modulo del fabricante desde `syscall/` -- que es exactamente lo que
    /// la cabecera de este fichero prohibe, y lo que ya se rompio una vez en
    /// tres sitios.
    pub identidad: fn() -> Option<(u8, u8)>,
}

/// **Una lectura cruda de los contadores de energia.**
///
/// Vive aqui --con el contrato-- y no en el modulo del fabricante, porque es lo
/// que la firma del perfil promete. El tipo de un contrato pertenece al
/// contrato: si viviera en `ryzen_5_5600x`, todo el que quisiera implementar
/// otro perfil tendria que importar el de AMD para poder no ser AMD.
#[derive(Clone, Copy)]
pub struct EnergiaCruda {
    /// Contador del paquete. **Da la vuelta**: es un contador, no un total.
    pub paquete: u32,
    /// Contador del nucleo en el que se leyo.
    pub nucleo: u32,
    /// Un incremento vale `1 / 2^exp` julios. **Se lee del silicio**, no se
    /// declara: un exponente supuesto no da error, da vatios inventados.
    pub exp: u8,
}

/// The compiled-in profile. Today: Ryzen 5 5600X. On another bench this
/// is the single line that changes.
pub fn active() -> &'static CpuProfile {
    &super::ryzen_5_5600x::PROFILE
}
