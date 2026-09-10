//! **DORMIR EN VEZ DE GIRAR: lo que le faltaba a AXION para poder ENCENDER.**
//!
//! [carril]  ROJO      duerme un nucleo. Si no despierta, ese nucleo no vuelve
//!
//! [cuesta]  MAQUINA -- un obrero que se duerme y no despierta deja a
//!           `crew::reparte` esperando en su barrera **para siempre**, y eso no
//!           es un nucleo perdido: es la maquina colgada, porque el BSP espera
//!           a todos (L6e)
//!
//! [riesgo]  SILENCIO -- un despertar perdido no da fault ni excepcion. El
//!           nucleo simplemente no vuelve, y lo unico que se ve es que el
//!           reparto no termina. Por eso este fichero **solo duerme cuando
//!           puede garantizar el despertar** -- ver el plazo (L6f)
//!
//! # *** EL PROBLEMA, DICHO POR `crew.rs` ANTES DE QUE EXISTIERA LA SOLUCION
//!
//! ```text
//!    "Un obrero en espera GIRA (`pause`), no duerme. Sacarlo de `hlt` pediria
//!     una IPI, y para atender una IPI un AP necesita GS por-CPU y su propia
//!     TSS -- que es justo el trabajo que este modulo evita. Consecuencia real
//!     y medible: con los doce en pie, once nucleos giran al 100 % y la maquina
//!     consume como si estuviera trabajando."
//! ```
//!
//! ** Y ahi esta la trampa que MONITOR/MWAIT deshace: **el obrero no espera una
//! interrupcion. Espera UNA ESCRITURA EN MEMORIA** -- que el BSP incremente
//! `RONDA`. `hlt` solo sabe despertar por interrupcion; `MWAIT` sabe despertar
//! **por escritura**, que es justo lo que hay.
//!
//! ```text
//!    hlt     duerme -> despierta con una IPI  -> pide GS por-CPU y TSS
//!    MWAIT   duerme -> despierta con un STORE -> no pide NADA
//! ```
//!
//! *** No es que MWAIT sea "la version buena de hlt". Es que **espera lo que
//! aqui de verdad se espera**. `hlt` era la herramienta equivocada, y por eso
//! su precio era tan alto.
//!
//! # ** EL PLAZO, Y POR QUE SIN EL NO SE DUERME
//!
//! `MWAIT` a secas no tiene despertador: si el `MONITOR` se rompe por lo que
//! sea --otra escritura en la misma linea, una transicion que lo desarma-- el
//! nucleo se queda ahi. Y un obrero que no vuelve **cuelga el reparto entero**,
//! porque `crew::reparte` espera a que esten todas las partes.
//!
//! AMD tiene la respuesta en el silicio: **`MWAITX`**, la variante con TIMEOUT
//! en `EBX`. El nucleo duerme, y si nadie escribe, **se despierta solo**.
//!
//! ```text
//!    hay MONITORX   -> MWAITX con plazo. Duerme, y despierta pase lo que pase
//!    solo MONITOR   -> NO SE DUERME. Se gira, como hasta hoy
//!    ninguno        -> NO SE DUERME
//! ```
//!
//! [!] La segunda fila es la decision de este fichero, y es deliberada: sin
//! plazo, dormir cambia *"once nucleos gastan de mas"* por *"la maquina puede
//! colgarse"*. **Se cambia un coste por un riesgo, y el coste era el que se
//! podia ver.**
//!
//! > Un ahorro que puede colgar la maquina no es un ahorro. Es una apuesta con
//! > la factura de la luz de premio.
//!
//! Este Ryzen 5 5600X es Zen 3 y trae `MONITORX` (CPUID 0x8000_0001, ECX bit
//! 29), asi que en la maquina del dueno se duerme de verdad. En una que no lo
//! traiga, esto no hace nada -- y lo dice.
//!
//! # El patron, y el orden NO es de gusto
//!
//! ```text
//!    1. MONITOR sobre la direccion      arma la vigilancia
//!    2. VOLVER A MIRAR el valor         *** y aqui esta todo
//!    3. MWAITX                          duerme
//! ```
//!
//! *** El paso 2 es el que hace correcto el patron. Entre que se decide dormir
//! y que se arma la vigilancia, el BSP puede haber publicado ya la ronda; sin
//! ese segundo vistazo, el obrero se duerme **con el trabajo delante** y no lo
//! ve hasta la siguiente. Con `MWAITX` eso costaria un plazo; sin el, seria
//! para siempre.
//!
//! Ver el apartado 5 de `docs/maestro/AXION_MAESTRO.md`, y `girando()` en
//! `smp/mod.rs`, que es el numero con el que se mide si esto sirvio.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// El plazo del `MWAITX`, en ticks del TSC.
///
/// ** Es DESPERTARSE, no trabajar: el obrero da una vuelta al bucle, ve que no
/// hay ronda nueva, y se vuelve a dormir. A 3,7 GHz esto son ~0,27 ms, o sea
/// unos 3.700 despertares por segundo y nucleo.
///
/// *** Y ese numero comparado con lo que sustituye no es un coste: es la
/// diferencia entre **girar al 100 %** y despertarse tres mil veces para mirar
/// un `u32`. El plazo corto se elige a proposito -- vale mas despertar de mas
/// que arriesgarse a que `PARAR` tarde en verse.
const PLAZO_TICKS: u32 = 1_000_000;

/// Veces que un obrero se ha dormido de verdad. `0` = no hay `MONITORX`, o
/// nadie ha esperado todavia.
static DORMIDAS: AtomicU64 = AtomicU64::new(0);

/// Si el silicio trae `MONITORX`. Se resuelve UNA vez, en el arranque.
static SE_PUEDE: AtomicU64 = AtomicU64::new(SIN_MIRAR);
const SIN_MIRAR: u64 = 0;
const NO: u64 = 1;
const SI: u64 = 2;

/// **Se puede dormir en esta maquina?** Lo contesta el silicio, no una opcion.
///
/// [!] Se exige `MONITORX` y no solo `MONITOR`: ver el apartado del plazo en la
/// cabecera. Sin despertador no se duerme.
pub fn se_puede() -> bool {
    match SE_PUEDE.load(Ordering::Relaxed) {
        SI => true,
        NO => false,
        _ => {
            // ** `leer()` hace CPUID, asi que se hace UNA vez y se guarda:
            // esto lo pregunta un bucle que gira miles de veces por segundo.
            use crate::ring0::cpu_vendor::features::{silicon, Feat};
            let hay = silicon::has(Feat::Monitorx, &silicon::leer());
            SE_PUEDE.store(if hay { SI } else { NO }, Ordering::Relaxed);
            hay
        }
    }
}

/// **Espera a que `celda` deje de valer `visto`, sin girar.**
///
/// Vuelve cuando el valor cambio, cuando venci el plazo, o cuando al silicio le
/// dio la gana -- las tres son respuestas validas, y por eso quien llama tiene
/// que estar dentro de un bucle que vuelva a mirar. **Esto no promete que algo
/// cambio: promete no haber quemado un nucleo mientras tanto.**
///
/// Si la maquina no puede dormir, gira una vez y vuelve. Asi quien llama no
/// tiene que preguntar.
///
/// # Seguridad
///
/// `celda` tiene que apuntar a memoria valida y viva mientras dure la espera.
/// En su unico llamador es un `static`, o sea que vive lo que vive el kernel.
pub fn esperar(celda: &AtomicU32, visto: u32) {
    if !se_puede() {
        core::hint::spin_loop();
        return;
    }
    let dir = celda as *const _ as usize;
    unsafe {
        // 1. Armar la vigilancia sobre la linea de esa direccion.
        //    ECX = extensiones (0), EDX = pistas (0).
        core::arch::asm!(
            "monitor",
            in("rax") dir,
            in("ecx") 0,
            in("edx") 0,
            options(nostack, preserves_flags),
        );
        // 2. *** VOLVER A MIRAR. Si el BSP publico entre la decision y el
        //    `monitor`, el trabajo ya esta ahi y dormirse seria perderlo.
        if celda.load(Ordering::SeqCst) != visto {
            return;
        }
        // 3. Dormir con plazo. `ECX` bit 1 = usar el `EBX` como timeout;
        //    `EAX` = 0 pide el estado mas ligero, que es el que despierta
        //    antes -- aqui interesa reaccionar, no ahorrar el ultimo vatio.
        // [!] `rbx` NO SE PUEDE PEDIR: LLVM lo usa por dentro y el
        // compilador lo rechaza de plano. Asi que se salva a mano alrededor
        // de la instruccion -- es el patron de siempre para `cpuid` y
        // compania, y aqui hace falta porque `mwaitx` lee el plazo de `ebx`.
        core::arch::asm!(
            "mov {salvo}, rbx",
            "mov ebx, {plazo:e}",
            "mwaitx",
            "mov rbx, {salvo}",
            salvo = out(reg) _,
            plazo = in(reg) PLAZO_TICKS,
            in("eax") 0,
            in("ecx") 2,
            options(nostack, preserves_flags),
        );
    }
    DORMIDAS.fetch_add(1, Ordering::Relaxed);
}

/// **Cuantas veces se durmio un obrero de verdad.**
///
/// Es el numero que convierte *"ahora deberia gastar menos"* en un dato. Si
/// sale CERO con los doce en pie, o no hay `MONITORX` o nadie llego a esperar
/// -- y las dos cosas se arreglan en sitios distintos.
pub fn dormidas() -> u64 {
    DORMIDAS.load(Ordering::Relaxed)
}
