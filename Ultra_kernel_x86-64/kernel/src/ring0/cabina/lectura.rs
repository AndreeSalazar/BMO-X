//! **LA LECTURA** -- lo que Ring 3 pregunta del anillo de CABINA.
//!
//! [carril]  AMARILLO  lee lo apuntado; no apunta ni pinta
//! [consumo] NADA      contesta cuando Ring 3 pregunta (`TASK_OP_CABINA_*`)
//!
//! Vivia al final de `cockpit.rs` y los dos contadores en `blackbox.rs` (L8b,
//! 2026-09-13). El cockpit y la caja negra se fueron al MIRADOR porque leen a
//! todo el kernel; esto solo lee el anillo, asi que se queda en el registro --
//! y el syscall, que lo usa, sigue llamando a `cabina`.

use super::*;

/// Total de eventos grabados desde el arranque (puede exceder el anillo).
pub fn event_total() -> u64 { unsafe { EV_TOTAL } }
/// Eventos perdidos por reentrancia. Deberia ser 0; si no lo es, algo falto
/// durante un fault y la bitacora lo dice en vez de callarlo.
pub fn event_lost() -> u64 { unsafe { EV_LOST } }

// =====================================================================
//  CABINA A RING 3 -- mirar TODO sin poder tocar nada
// =====================================================================
//
// Hasta hoy CABINA se pintaba **solo desde el shell de Ring 0**, y desde que el
// escritorio es el arranque eso significa que casi nunca se ve. Lo que F11
// ensena es el KLOG, que es otra cosa: transcripcion en texto plano, 96 bytes
// por linea y **sin severidad**. La linea que dice si el SMP levanto los doce
// nucleos existe con su color y su capa, y a Ring 3 le llegaba en gris.
//
// Esto lo abre. Y **no es "ir a Ring 0"**: aqui no se ejecuta nada
// privilegiado, no se concede ningun objeto y no hay una sola operacion que
// escriba. El compositor sigue siendo un proceso con sus capabilities
// contadas, y lo unico que hace es PREGUNTAR -- igual que con `info`, el klog y
// la autopsia.
//
// En un sistema de capabilities **ver y poder son cosas separadas**, y que se
// pueda mirar TODO sin poder tocar nada es la mitad interesante de la
// transparencia que este proyecto declara. Un "terminal privilegiado" que de
// verdad ejecutara en Ring 0 tiraria el modelo a la basura para conseguir algo
// que se puede tener sin romper nada: mirar.

/// Campos de `TASK_OP_CABINA_INFO`. Son una TABLA, igual que `OP_INFO`:
/// anadir un dato es una fila, no una operacion nueva.
pub const CABINA_TOTAL: u64 = 0x00;
pub const CABINA_LOST: u64 = 0x01;
pub const CABINA_AVAILABLE: u64 = 0x02;
/// Los cinco de un evento concreto. `arg1` = cual (0 = el mas reciente).
pub const CABINA_SEVERITY: u64 = 0x03;
pub const CABINA_LAYER: u64 = 0x04;
pub const CABINA_VALUE: u64 = 0x05;
pub const CABINA_SEQ: u64 = 0x06;
pub const CABINA_TICK: u64 = 0x07;
/// De que INTENTO salio el evento. `0` = de ninguno. Ver `bmo-abi`: es lo que
/// permite que la ventana de Ring 3 filtre por ACCION y no solo por gravedad.
pub const CABINA_ATTEMPT: u64 = 0x08;
// ** EL BARRIDO. Ver `cabina/radar.rs`: cuenta lo que el anillo pierde.
pub const CABINA_BARRIDO_CUENTA: u64 = 0x10;
pub const CABINA_BARRIDO_ULTIMO: u64 = 0x11;
pub const CABINA_VENTANA: u64 = 0x12;
pub const CABINA_CLASES_FUERA: u64 = 0x13;
/// **Cuantos hubo de esta clase en el ULTIMO SEGUNDO cerrado.** Ver
/// `radar::ritmo`: un total no distingue una emergencia de un recuerdo.
pub const CABINA_BARRIDO_RITMO: u64 = 0x14;
/// Cuantas ventanas de un segundo se han cerrado. **Sin esto un ritmo de cero
/// es ambiguo**: puede ser "no paso nada" o "todavia no ha pasado un segundo".
pub const CABINA_VENTANAS: u64 = 0x15;

/// Que texto se pide en `TASK_OP_CABINA_TEXTO`.
pub const CABINA_TXT_MODULE: u64 = 0x00;
pub const CABINA_TXT_MESSAGE: u64 = 0x01;

/// Cuantos eventos se pueden leer AHORA. Nunca mas que el anillo.
pub fn disponibles() -> u64 {
    let total = unsafe { EV_TOTAL };
    if total > EVENT_RING as u64 { EVENT_RING as u64 } else { total }
}

/// Un dato numerico de CABINA. `n` = que evento (0 = el mas reciente).
///
/// Devuelve `None` para un campo que no existe, que el syscall traduce a "no
/// soportado" -- y no 0, que seria indistinguible de un evento con valor cero.
/// **El `seq` mas bajo que sigue dentro del anillo.**
///
/// Todo evento con un `seq` menor que este existio y **ya no se puede leer**.
/// Con menos de 48 eventos desde el arranque no se ha caido nada todavia, y
/// entonces la ventana empieza en el 1.
///
/// [!] `EV_SEQ` es el ULTIMO entregado, no el siguiente. Con 48 eventos justos,
/// el mas viejo es el `1` -- y un `+1` de mas aqui diria que el primero se cayo
/// cuando sigue ahi.
fn primer_seq_visible() -> u64 {
    let total = event_total();
    if total <= super::ring::EVENT_RING as u64 {
        1
    } else {
        total - super::ring::EVENT_RING as u64 + 1
    }
}

pub fn campo(campo: u64, n: u64) -> Option<u64> {
    match campo {
        CABINA_TOTAL => Some(event_total()),
        CABINA_LOST => Some(event_lost()),
        CABINA_AVAILABLE => Some(disponibles()),
        // ** LOS SEIS DEL BARRIDO. Van ANTES del `_`, que resuelve un evento
        // del anillo: estos NO son de un evento, son de todos los que hubo --
        // incluidos los que el anillo ya no tiene.
        CABINA_BARRIDO_CUENTA => Some(super::radar::cuenta((n >> 8) as usize, (n & 0xFF) as usize)),
        CABINA_BARRIDO_ULTIMO => Some(super::radar::ultimo((n >> 8) as usize, (n & 0xFF) as usize)),
        CABINA_VENTANA => Some(primer_seq_visible()),
        CABINA_CLASES_FUERA => Some(super::radar::clases_fuera_de_ventana(primer_seq_visible())),
        CABINA_BARRIDO_RITMO => Some(super::radar::ritmo((n >> 8) as usize, (n & 0xFF) as usize)),
        CABINA_VENTANAS => Some(super::radar::ventanas()),
        _ => {
            let ev = event_back(n as usize)?;
            match campo {
                CABINA_SEVERITY => Some(ev.severity as u64),
                CABINA_LAYER => Some(ev.layer as u64),
                CABINA_VALUE => Some(ev.value),
                CABINA_SEQ => Some(ev.seq),
                CABINA_TICK => Some(ev.tick_ns),
                CABINA_ATTEMPT => Some(ev.intento as u64),
                _ => None,
            }
        }
    }
}

/// Ocho bytes del modulo o del mensaje del evento `n`, empaquetados
/// little-endian. `trozo` numera de 8 en 8; el cero corta.
///
/// Mismo formato que `TASK_OP_RUTA` y que el klog, y por la misma razon: **la
/// superficie congelada no acepta punteros**, asi que el texto viaja por valor.
pub fn texto(n: u64, cual: u64, trozo: u64) -> u64 {
    let ev = match event_back(n as usize) {
        Some(e) => e,
        None => return 0,
    };
    let bytes: &[u8] = match cual {
        CABINA_TXT_MODULE => &ev.module,
        CABINA_TXT_MESSAGE => &ev.msg,
        _ => return 0,
    };
    let base = (trozo as usize) * 8;
    let mut w = [0u8; 8];
    for i in 0..8 {
        let j = base + i;
        if j >= bytes.len() || bytes[j] == 0 {
            break;
        }
        w[i] = bytes[j];
    }
    u64::from_le_bytes(w)
}
