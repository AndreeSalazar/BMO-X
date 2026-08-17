//! **Cuanto consume esta maquina, en milivatios.**
//!
//! Escalon 2 de la seccion 9 de `docs/maestro/AXION_MAESTRO.md`, y el que convierte una
//! frase de AXION en una medida:
//!
//! > *"un obrero que espera no duerme, GIRA. Con los doce en pie, once nucleos
//! > al 100% y la maquina consume como si trabajara."*
//!
//! Eso llevaba escrito desde el 08-11 **sin un solo numero al lado**. Con esto,
//! `smp stop` deja de ser un acto de fe y pasa a tener un antes y un despues.
//!
//! # El reparto, que es el mismo de siempre
//!
//! | | |
//! |---|---|
//! | `cpu_vendor/ryzen_5_5600x/energia.rs` | **DONDE** estan los contadores. Es de AMD, y por eso vive en el perfil |
//! | este fichero | **QUE HACER** con ellos: restar, dividir y contestar milivatios |
//!
//! La aritmetica no es de ningun fabricante. El dia que haya un segundo perfil,
//! este fichero no se toca.
//!
//! # Y otra vez: son CONTADORES
//!
//! Igual que MPERF/APERF. Leerlos una vez da el total desde que arranco la
//! maquina; **el consumo de AHORA es una diferencia entre dos instantes**. Por
//! eso hace falta guardar la lectura anterior Y el TSC de esa lectura: sin el
//! tiempo no hay vatios, hay julios.
//!
//! [!] Los contadores son de **32 bits y dan la vuelta**. A 65 W con una unidad
//! de ~15 microjulios, la vuelta llega en unos 16 minutos. `wrapping_sub` da la
//! diferencia correcta al dar la vuelta; lo que no arregla es dar DOS vueltas
//! entre lecturas, y por eso el panel tiene que preguntar seguido.

use crate::ring0::cpu_vendor::profile;

/// **El lector que el PERFIL declara**, o `None` si este silicio no expone nada.
///
/// Este fichero no nombra a ningun fabricante, y ese es el punto: la aritmetica
/// de restar dos contadores y dividir por el tiempo es la misma en un AMD, un
/// Intel y un ARM. El dia que haya un segundo perfil, aqui no se toca una linea.
fn lector() -> Option<fn() -> Option<profile::EnergiaCruda>> {
    profile::active().energia
}

/// Se pudo leer alguna vez? Se resuelve en [`init`].
static mut HAY: bool = false;
/// Lectura anterior, para restar.
static mut PREV_PKG: u32 = 0;
static mut PREV_NUC: u32 = 0;
static mut PREV_TSC: u64 = 0;
static mut EXP: u8 = 0;
/// Los ultimos milivatios calculados: `(paquete, nucleo)`.
static mut ULTIMO: (u64, u64) = (0, 0);

/// **Pregunta si este CPU sabe decir lo que gasta, y se apunta la respuesta.**
///
/// Una vez, en el arranque. Ver [`crate::ring0::cpu::frequency::init`]: mismo
/// motivo, un panel que se repinta no puede costar una comprobacion cada vez.
pub fn init() {
    let Some(leer) = lector() else {
        crate::ring0::cabina::warn("cpu", "el perfil no declara lector de energia", 0);
        return;
    };
    match leer() {
        Some(e) => unsafe {
            HAY = true;
            EXP = e.exp;
            PREV_PKG = e.paquete;
            PREV_NUC = e.nucleo;
            PREV_TSC = crate::ring0::task::scheduler::rdtsc();
            // La unidad se DICE. Es el unico hecho que se le pregunto al chip, y
            // si algun dia los vatios salen raros, este numero es lo primero que
            // hay que mirar -- un exponente distinto son vatios distintos por un
            // factor de dos.
            crate::ring0::cabina::count("cpu", "RAPL disponible; 1 unidad = 1/2^N julios, N", e.exp as u64);
        },
        None => {
            // Se dice, y no se calla: sin esto el panel mostraria 0 W y no habria
            // forma de distinguir "no lo soporta" de "no consume nada", que es
            // una frase que no puede ser verdad.
            crate::ring0::cabina::warn("cpu", "sin RAPL creible: el consumo no se puede medir", 0);
        }
    }
}

/// Lo soporta esta maquina?
pub fn disponible() -> bool {
    unsafe { HAY }
}

/// **Milivatios desde la ultima vez que se pregunto**: `(paquete, nucleo)`.
///
/// `(0, 0)` si no se puede medir todavia. Como con la frecuencia, el cero es
/// deliberado y no se sustituye por un TDP de catalogo: un numero plausible
/// inventado se cree y se usa para decidir; un cero se investiga.
pub fn medir() -> (u64, u64) {
    if !unsafe { HAY } {
        return (0, 0);
    }
    let Some(leer) = lector() else { return (0, 0) };
    let Some(e) = leer() else { return (0, 0) };
    let ahora = crate::ring0::task::scheduler::rdtsc();
    let (ppkg, pnuc, ptsc, exp) = unsafe { (PREV_PKG, PREV_NUC, PREV_TSC, EXP) };
    unsafe {
        PREV_PKG = e.paquete;
        PREV_NUC = e.nucleo;
        PREV_TSC = ahora;
    }

    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz == 0 {
        return (0, 0);
    }
    let dt = ahora.wrapping_sub(ptsc);
    // Milisegundos transcurridos. Por debajo de 10 ms el cociente es ruido: un
    // contador de energia se actualiza cada cierto tiempo, no de forma continua,
    // y preguntarle mas rapido que su refresco da picos que no ocurrieron.
    let ms = dt / (hz / 1000).max(1);
    if ms < 10 {
        return unsafe { ULTIMO };
    }

    let r = (
        milivatios(e.paquete.wrapping_sub(ppkg), exp, ms),
        milivatios(e.nucleo.wrapping_sub(pnuc), exp, ms),
    );
    unsafe { ULTIMO = r };
    r
}

/// **De incrementos de contador a milivatios**, sin coma flotante.
///
/// ```text
///    microjulios  = incrementos * 1_000_000 / 2^exp
///    milivatios   = microjulios / milisegundos
/// ```
///
/// El orden importa, y es el mismo cuidado que en `frequency::medir`:
/// multiplicar por un millon ANTES de desplazar. Al reves, `incrementos >> 16`
/// vale 0 para cualquier consumo normal y el resultado seria siempre cero --
/// pareciendo un CPU apagado en vez de una division mal puesta.
///
/// No desborda: `incrementos` es de 32 bits, asi que el producto tope es
/// `2^32 * 10^6`, unos `4x10^15`. Cabe holgado en 64.
fn milivatios(incrementos: u32, exp: u8, ms: u64) -> u64 {
    if ms == 0 || exp >= 64 {
        return 0;
    }
    let microjulios = (incrementos as u64).saturating_mul(1_000_000) >> exp;
    microjulios / ms
}

#[cfg(test)]
mod pruebas_de_lectura {
    // [!] Este crate es el kernel: `cargo test` no corre aqui (aporta el panic
    // handler, ver `bmo-uaudio/src/stream.rs`). La aritmetica de
    // [`super::milivatios`] es pura y se comprueba a mano con `rustc --test`
    // copiando la funcion, o se mueve a un crate probable el dia que haya otro
    // consumidor. Se deja dicho en vez de fingir cobertura.
}
