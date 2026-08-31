//! Professional CPU-profile contract: swapping the CPU (or the vendor)
//!
//! [carril]  AMARILLO  el contrato de perfil; cambiar de CPU pasa por aqui
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

    // -- ** LO QUE SUBE DESDE EL 2026-08-25: no el numero, la CONFIANZA -----
    //
    // El contrato subia cuatro numeros y ninguno decia de donde venia. Con eso,
    // un `27 fisicos / 54 logicos` sube igual de limpio que un 6/12: el que lo
    // recibe no tiene forma de saber que uno se midio y el otro se supuso.
    /// **Hilos por nucleo, medidos.** `0` = no se pudo medir, y entonces
    /// `nucleos` es el mismo numero que `hilos` en vez de una division
    /// inventada. Un cero aqui es una respuesta, no un fallo.
    pub hilos_por_nucleo: u32,
    /// **Las dos fuentes del silicio no coinciden.** Quien lo lea tiene que
    /// desconfiar del numero entero, no elegir el que le guste.
    pub discrepan: bool,
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

    // -- ** EL TOPE DEL PERFIL (2026-08-25) --------------------------------
    //
    // Es la ley 24 cobrando lo que promete. Un driver generico **no puede**
    // saber que 54 hilos esta mal: cualquier numero es plausible cuando no
    // sabes de que chip hablas. Un PERFIL si puede, porque su primera linea
    // dice `Ryzen 5 5600X`.
    //
    // [!] Y NO CORRIGE: avisa. Es la misma doctrina que `xsave_componentes`
    // --se hardcodean los CONTRATOS, se le preguntan los HECHOS al silicio--.
    // Un perfil que sobreescribiera lo que midio el CPU seria un kernel que
    // miente con mas confianza, que es peor que uno que se equivoca.
    /// Lo que este chip EXACTO tiene: `(nucleos, hilos)`. `None` = el perfil no
    /// se atreve a decirlo, y entonces no hay nada contra que carear.
    pub topologia_esperada: Option<(u32, u32)>,

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

/// **EL CAREO DE LA TOPOLOGIA, en el arranque y sin que nadie lo pida.**
///
/// # Por que existe: el 27/54 del 2026-08-25
///
/// El escritorio contesto `27 fisicos / 54 logicos` en un 6/12, y **ningun
/// aviso salio**. No porque el careo no existiera --`plat::smp` compara CPUID
/// contra la MADT desde hace semanas-- sino porque vivia **dentro de
/// `despertar()`**, y a `despertar()` solo se llega tecleando `smp`.
///
/// ```text
///    el careo existia   ->  pero solo corria si el DUENO lo pedia
///    el numero malo     ->  se ensenaba en CADA panel, desde el arranque
/// ```
///
/// *** **Una comprobacion que hay que invocar no protege del caso en el que
/// nadie la invoca.** Y ese es justo el caso en el que hace falta: el dueno mira
/// el panel porque quiere el dato, no porque sospeche del dato.
///
/// # Los tres testigos, y que significa que discrepen
///
/// ```text
///    CPUID.0B     lo que el silicio TIENE          manda
///    CPUID.1      el conteo heredado               CAREA
///    la MADT      lo que el firmware DECLARA       CAREA
///    el PERFIL    de que chip es esto              DESMIENTE
/// ```
///
/// Ninguno corrige a otro. **Los cuatro hablan y el que lee decide**, que es lo
/// contrario de lo que habia: un solo numero, sin origen, que se creia entero.
pub fn carear_topologia() {
    use crate::ring0::cabina;

    let p = active();
    let Some(n) = (p.nucleos)() else {
        cabina::warn("cpu", "sin topologia: el perfil no ha contestado", 0);
        // Sin topologia no hay nada que carear, y eso ES una duda: se marcan
        // los cuatro bits. El silencio y "no se pudo preguntar" no se pueden
        // ver igual desde Ring 3.
        DUDA.store(0b1111, Ordering::Release);
        return;
    };

    let mut duda = 0u32;

    // -- 1. Las dos fuentes del SILICIO ------------------------------------
    if n.discrepan {
        duda |= DUDA_CPUID;
        cabina::warn("cpu", "CPUID se contradice: hoja 0x0B contra la heredada", n.hilos as u64);
    }
    if n.hilos_por_nucleo == 0 {
        duda |= DUDA_SIN_MEDIR;
        // No es un fallo: es "no se pudo medir". Pero tiene que verse, porque
        // significa que `nucleos` NO es una division sino una copia de `hilos`.
        cabina::warn("cpu", "hilos por nucleo SIN MEDIR: nucleos no es fiable", n.nucleos as u64);
    }

    // -- 2. *** EL PERFIL DESMINTIENDO AL SILICIO (la ley 24 cobrando) -----
    //
    // Aqui es donde un 54 se cae. Y no se corrige a 12: se GRITA. Corregirlo
    // dejaria un sistema que ensena el numero bueno y esconde que su fuente
    // esta rota -- que es exactamente como se llego hasta aqui.
    if let Some((nucleos_ok, hilos_ok)) = p.topologia_esperada {
        if n.hilos != hilos_ok || n.nucleos != nucleos_ok {
            duda |= DUDA_PERFIL;
            cabina::fault("cpu", "el silicio NO dice lo que este perfil sabe que es", n.hilos as u64);
            cabina::count("cpu", "el perfil esperaba estos hilos", hilos_ok as u64);
        }
    }

    // -- 3. El FIRMWARE, que es el tercer testigo y el unico independiente --
    //
    // ** Esto ya lo hacia `plat::smp`, y se hace TAMBIEN aqui a proposito: alli
    // contesta a "a quien despierto", aqui a "me puedo creer el numero". Son
    // dos preguntas distintas con la misma comparacion, y la segunda no puede
    // depender de que alguien haga la primera.
    if let Some(c) = crate::ring0::plat::madt::censo() {
        let declarados = c.ids().len() as u32;
        if declarados != n.hilos {
            duda |= DUDA_MADT;
            cabina::warn("cpu", "la MADT declara otros hilos que CPUID", declarados as u64);
        }
    } else {
        cabina::warn("cpu", "sin MADT: el silicio se queda sin quien lo contradiga", 0);
    }

    // -- 4. Y la foto, siempre, discrepen o no ------------------------------
    //
    // [!] Se apunta AUNQUE todo cuadre. Una linea que solo aparece cuando algo
    // va mal no deja ver que el dia anterior iba bien -- y esa comparacion es
    // justo la que faltaba el 25-08, cuando el mismo codigo habia dicho 12.
    cabina::count("cpu", "hilos", n.hilos as u64);
    cabina::count("cpu", "nucleos fisicos", n.nucleos as u64);
    cabina::count("cpu", "hilos por nucleo", n.hilos_por_nucleo as u64);

    // ** Y el veredicto se GUARDA, porque el panel del escritorio lo va a pedir
    // en cada repintado y no puede volver a enumerar la MADT por fotograma.
    // Ver `INFO_CPU_TOPOLOGIA_DUDA`.
    DUDA.store(duda, Ordering::Release);
}

/// Los cuatro bits de [`duda`], y el mismo orden que declara el ABI.
pub const DUDA_CPUID: u32 = 1 << 0;
pub const DUDA_SIN_MEDIR: u32 = 1 << 1;
pub const DUDA_PERFIL: u32 = 1 << 2;
pub const DUDA_MADT: u32 = 1 << 3;

/// [!] Arranca en `DUDA_SIN_MEDIR` **a proposito**: mientras `carear_topologia`
/// no haya corrido, la respuesta honesta no es "todo bien" sino "todavia no se
/// ha mirado". Un cero por defecto habria hecho que el panel afirmara que los
/// testigos coinciden **antes de que nadie los hubiera comparado** -- que es
/// exactamente la clase de silencio que costo el 27/54.
static DUDA: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(DUDA_SIN_MEDIR);

use core::sync::atomic::Ordering;

/// El mapa de bits del ultimo careo. Lo sirve `INFO_CPU_TOPOLOGIA_DUDA`.
pub fn duda() -> u32 {
    DUDA.load(Ordering::Acquire)
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
