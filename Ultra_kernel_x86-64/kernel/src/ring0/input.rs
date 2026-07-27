//! `KIND_INPUT` — el ratón como capability.
//!
//! El kernel ya sabía dónde está el ratón: `dev::usb` acumula los deltas del
//! HID desde que el xHCI lo enumeró. Lo que no había era forma de que ese dato
//! **saliera de Ring 0**, así que el puntero era un número que sólo servía para
//! el diagnóstico. Esto lo abre.
//!
//! ## Por qué esto sí es del kernel y el cursor no
//!
//! Leer el HID es tocar hardware: transferencias xHCI, endpoints, reintentos.
//! Eso vive en Ring 0 porque no hay otro sitio donde pueda vivir todavía.
//!
//! **Dibujar el cursor no.** El cursor es una decisión de aspecto —forma,
//! color, si tiene sombra, si cambia sobre un borde— y ninguna de esas
//! decisiones tiene nada que hacer dentro de un kernel. Aquí sólo sale un par
//! de coordenadas y un mapa de botones; el compositor decide qué pinta con eso.
//! La misma línea que separa `KIND_FRAMEBUFFER` de un `DRAW_RECT`.
//!
//! ## Exclusiva, como la pantalla
//!
//! Un solo proceso la tiene: el multiplexor de entrada. Cuando `services/input`
//! exista de verdad, será él quien la reclame y quien reparta los eventos entre
//! el compositor y las aplicaciones. Hoy la reclama el compositor directamente,
//! que es lo honesto mientras no haya nadie más a quien repartir.
//!
//! ★ Y como con la pantalla: hoy la reclama el primero que la pide. La
//! autoridad correcta es la bandera del BEF verificada por el gate.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::ring0::cap;

const SIN_DUENO: u32 = u32::MAX;
static DUENO: AtomicU32 = AtomicU32::new(SIN_DUENO);

/// Ya la tiene otro proceso.
pub const ERROR_OCUPADO: u32 = 16;

/// Estado del puntero, empaquetado: `(x << 32) | (y << 16) | botones`.
///
/// Una llamada por fotograma en vez de tres. `x` e `y` caben en 16 bits con
/// holgura —4096 px es más pantalla de la que hay— y los botones son una
/// máscara de bits.
pub const INPUT_OP_PUNTERO: u64 = 0x01;

/// Cuántos eventos HID se han visto. Sirve para distinguir "el ratón no se
/// mueve" de "el ratón no llega": si esto no sube, el problema está en el USB,
/// no en el compositor.
pub const INPUT_OP_EVENTOS: u64 = 0x02;

pub fn reclamar(pid: u32) -> Result<u64, u32> {
    if DUENO
        .compare_exchange(SIN_DUENO, pid, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ERROR_OCUPADO);
    }
    match cap::grant(pid, cap::KIND_INPUT, cap::RIGHT_READ, 0) {
        Some(h) => {
            crate::ring0::cabina::info("input", "raton cedido a Ring 3", pid as u64);
            Ok(h)
        }
        None => {
            DUENO.store(SIN_DUENO, Ordering::SeqCst);
            Err(cap::ERROR_PERMISSION_DENIED)
        }
    }
}

/// Lo llama `cap::revoke_all`: si el dueño muere, la entrada vuelve al kernel.
pub fn proceso_muerto(pid: u32) {
    let _ = DUENO.compare_exchange(pid, SIN_DUENO, Ordering::SeqCst, Ordering::SeqCst);
}

/// Despacho de las operaciones. `None` = operación que no existe.
pub fn operacion(operacion: u64) -> Option<u64> {
    let (x, y, botones, eventos) = crate::ring0::dev::usb::puntero();
    match operacion {
        INPUT_OP_PUNTERO => {
            // Se recorta al panel AQUÍ, que es donde se sabe de qué tamaño es.
            // Un acumulador de deltas sin tope se va a valores absurdos con
            // dos pasadas de ratón y el compositor tendría que recortarlo
            // igual, sólo que sin saber contra qué.
            let (ancho, alto) = unsafe {
                (
                    crate::info::FB_WIDTH.max(1) - 1,
                    crate::info::FB_HEIGHT.max(1) - 1,
                )
            };
            let cx = x.clamp(0, ancho as i32) as u64;
            let cy = y.clamp(0, alto as i32) as u64;
            Some((cx << 32) | (cy << 16) | botones as u64)
        }
        INPUT_OP_EVENTOS => Some(eventos as u64),
        _ => None,
    }
}
