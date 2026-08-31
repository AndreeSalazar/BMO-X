//! **CARRIL VERDE** -- la geometria y los numeros del contrato.
//!
//! [cuesta]  NADA -- de aqui no sale ni un cambio de propiedad ni un mapeo. Se
//!           contesta donde quedo, cuanto mide y quien la tiene. Equivocarse
//!           pinta torcido, y lo ve el que pregunto.
//!
//! [riesgo]  -- ninguno declarado.
//!
//! # *** POR QUE LOS `FB_OP_*` SON VERDES SIENDO LA FRONTERA
//!
//! Porque son **numeros de un contrato, no decisiones**. Una operacion que no
//! existe contesta `None` y Ring 3 se entera en el acto -- lo contrario de un
//! fallo callado. Y `operation` no toca ni un byte del framebuffer: lee cuatro
//! campos de `info` y hace dos desplazamientos.
//!
//! [!] `owner()` se lee SIN cerrojo y a proposito: lo llama el shell y lo
//! llamaria una pantalla de fallo. Un valor de hace un tick es aceptable para
//! contestar una pregunta; colgarse contestandola, no.

use core::sync::atomic::Ordering;
use crate::ring0::mm;
use super::roja::{NO_OWNER, OWNER};

/// Ya la tiene otro proceso.
pub const ERROR_BUSY: u32 = 16;

/// Esta maquina arranco sin GOP: no hay pantalla que ceder.
pub const ERROR_NO_SCREEN: u32 = 17;

// Operaciones sobre un handle KIND_FRAMEBUFFER.
//
// Cada una devuelve UN `u64` porque eso es lo que cabe en `BmoStatus.value`.
// Los campos que van juntos viajan empaquetados en vez de gastar una llamada
// por numero: son datos que se leen una vez al arrancar el compositor.

/// Direccion virtual (en el espacio del proceso) donde quedo mapeada.
pub const FB_OP_BASE: u64 = 0x01;

/// `(ancho << 32) | alto`, en pixeles.
pub const FB_OP_DIMS: u64 = 0x02;

/// `(stride << 32) | formato`. El stride va en PIXELES, no en bytes -- es el
/// mismo numero que usa el kernel, y convertirlo aqui seria inventar una
/// unidad distinta a los dos lados de la frontera.
pub const FB_OP_STRIDE: u64 = 0x03;

/// Bytes mapeados en total. Es lo que hace falta para un `rep stosd` que
/// llene la pantalla entera sin multiplicar nada.
pub const FB_OP_BYTES: u64 = 0x04;


/// Bytes que ocupa el framebuffer, redondeado a pagina.
pub(super) fn mapped_bytes() -> u64 {
    let alto = unsafe { crate::info::FB_HEIGHT } as u64;
    let stride = unsafe { crate::info::FB_STRIDE } as u64;
    let crudo = alto * stride * 4;
    (crudo + mm::PAGE - 1) & !(mm::PAGE - 1)
}


/// Pid del dueno actual, o `None`.
pub fn owner() -> Option<u32> {
    match OWNER.load(Ordering::SeqCst) {
        NO_OWNER => None,
        pid => Some(pid),
    }
}


/// Despacho de las operaciones sincronas sobre la capability ya resuelta.
/// `base` es el objeto que guarda la capability: la VA donde se mapeo.
pub fn operation(base: u64, operation: u64) -> Option<u64> {
    let (ancho, alto, stride, formato) = unsafe {
        (
            crate::info::FB_WIDTH as u64,
            crate::info::FB_HEIGHT as u64,
            crate::info::FB_STRIDE as u64,
            crate::info::FB_PIXEL_FORMAT as u64,
        )
    };
    match operation {
        FB_OP_BASE => Some(base),
        FB_OP_DIMS => Some((ancho << 32) | alto),
        FB_OP_STRIDE => Some((stride << 32) | formato),
        FB_OP_BYTES => Some(mapped_bytes()),
        _ => None,
    }
}


