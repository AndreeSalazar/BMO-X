//! **WHO HOLDS THE DISK** -- one owner at a time, and a count of the thefts.
//!
//! [carril]  ROJO      un dueno cada vez, y la cuenta de los robos
//!
//! === Why this is a file of its own ===
//!
//! Because it is not storage: it is a **lock with a memory**. AHCI here has one
//! command slot in play, so two tasks issuing at once would interleave into one
//! transfer that belongs to neither. `Testigo` is the token that prevents it,
//! and it releases in `Drop` so an early return cannot leak the disk.
//!
//! === The part that earns the file: it COUNTS ===
//!
//! `ESPERAS` and `ROBOS` are not debug leftovers. A lock that only works tells
//! you nothing; a lock that reports **how often somebody waited and how often
//! somebody had to take it** turns "the disk feels slow" into a number, and
//! turns "this never happens" into a claim that can be checked from CABINA.
//!
//! `CADA_CUANTO_MIRAR` is the same idea pointed at the other risk: a spin that
//! never looks up is a hang with no explanation.

use super::*;

/// Tid de quien tiene el disco ahora mismo. `0` = libre.
static DUENO: AtomicU32 = AtomicU32::new(0);
/// Cuantas veces hubo que esperar a que otro soltara. Es la medida de si esto
/// hacia falta: si nunca sube, es que nadie se solapaba; si sube, cada punto
/// era una lectura corrupta antes de existir esto.
static ESPERAS: AtomicU32 = AtomicU32::new(0);
/// Cuantas veces hubo que QUITARSELO a un dueno que ya no vive.
static ROBOS: AtomicU32 = AtomicU32::new(0);

/// `(esperas, robos)` desde el arranque.
pub fn cuentas_dueno() -> (u32, u32) {
    (ESPERAS.load(Ordering::Relaxed), ROBOS.load(Ordering::Relaxed))
}

/// El testigo del disco. Al soltarse, libera -- **por todos los caminos**,
/// incluido un `return` a mitad de la funcion.
///
/// Eso es exactamente el motivo de que sea un tipo y no un par de llamadas: la
/// version con `tomar()` y `soltar()` se olvida en el sexto `return` de
/// `read`, y el sintoma es un disco que deja de contestar para siempre.
/// `true` = este testigo es el que libera. Un anidado lleva `false`: soltar
/// desde dentro dejaria el disco libre **con la operacion de fuera a medias**,
/// que es justo lo que el dueno existe para impedir.
pub struct Testigo(bool);

impl Drop for Testigo {
    fn drop(&mut self) {
        if self.0 {
            DUENO.store(0, Ordering::Release);
        }
    }
}

/// Cada cuantas vueltas se pregunta si el dueno sigue vivo.
///
/// No en cada una: `pid_de` toma el candado del planificador, y hacerlo en un
/// bucle apretado es meterse en el camino de lo que estamos esperando. Cuatro
/// mil vueltas son microsegundos, y lo que se detecta --un dueno muerto-- no se
/// va a arreglar solo en ese rato.
const CADA_CUANTO_MIRAR: u64 = 4096;

/// Toma el disco. Espera si lo tiene otro; se lo quita si ese otro ya murio.
pub(super) fn tomar_disco() -> Testigo {
    let yo = crate::ring0::task::scheduler::current_tid().max(1);
    let mut vueltas = 0u64;
    loop {
        if DUENO.compare_exchange(0, yo, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            return Testigo(true);
        }
        let otro = DUENO.load(Ordering::Relaxed);
        if otro == yo {
            // ** Anidado: alguien de arriba ya lo tiene. No deberia pasar --las
            // funciones que lo toman no se llaman entre ellas-- asi que se dice
            // en vez de callarlo, y se devuelve un testigo que NO libera.
            crate::ring0::cabina::warn("disk", "peticion de disco anidada (no deberia)", yo as u64);
            return Testigo(false);
        }
        if vueltas == 0 {
            ESPERAS.fetch_add(1, Ordering::Relaxed);
        }
        vueltas += 1;
        // El dueno ya no existe? Entonces murio con el disco en la mano.
        if vueltas % CADA_CUANTO_MIRAR == 0
            && crate::ring0::task::scheduler::pid_de(otro).is_none()
            && DUENO.compare_exchange(otro, yo, Ordering::Acquire, Ordering::Relaxed).is_ok()
        {
            ROBOS.fetch_add(1, Ordering::Relaxed);
            crate::ring0::cabina::warn("disk", "el dueno del disco murio: se le quita", otro as u64);
            return Testigo(true);
        }
        core::hint::spin_loop();
    }
}
