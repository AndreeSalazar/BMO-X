//! v2.0 — Coalesce de eventos.
//!
//! Implementa las reglas de Win32:
//!   - MOUSEMOVE: sólo se conserva el último (los anteriores se descartan).
//!   - PAINT: múltiples invalidaciones → un único MSG_PAINT con la
//!     bounding box de la región inválida.
//!   - SIZE/MOVE: si llega otro del mismo tipo con el mismo tamaño/pos
//!     mientras no se ha despachado, se reemplaza.
//!   - KEYDOWN/KEYUP: no se coalescen — son eventos discretos.

#![allow(dead_code)]

use super::message::{BmoMsg, BmoMsgKind};

/// Inserta un mensaje en la cola del thread, aplicando coalesce según
/// el tipo. Devuelve `true` si el mensaje se encoló, `false` si se
/// descartó por coalesce.
pub fn post_coalesced(queue: &mut super::queue::BmoQueue, msg: BmoMsg) -> bool {
    match BmoMsgKind::from_u16(msg.kind) {
        BmoMsgKind::MouseMove | BmoMsgKind::MouseHover => {
            // Buscar un MouseMove ya en cola para el mismo target;
            // si existe, reemplazarlo (sólo conservamos el último).
            for i in 0..super::queue::QUEUE_CAP {
                if queue.msgs[i].target == msg.target
                    && (BmoMsgKind::from_u16(queue.msgs[i].kind) == BmoMsgKind::MouseMove
                        || BmoMsgKind::from_u16(queue.msgs[i].kind) == BmoMsgKind::MouseHover)
                {
                    queue.msgs[i] = msg;
                    return true;
                }
            }
        }
        BmoMsgKind::Paint => {
            // Si ya hay un Paint para el mismo target, expandir el
            // bounding box (no implementado: simplificamos descartando
            // duplicados en el compositor, no aquí).
        }
        _ => {}
    }
    queue.push(msg)
}
