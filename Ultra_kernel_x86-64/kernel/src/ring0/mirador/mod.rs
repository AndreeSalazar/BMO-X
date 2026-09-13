//! **EL MIRADOR** -- lo que CABINA ENSENA y VUELCA, separado de lo que APUNTA.
//!
//! [familia] mirador  nivel 13 -- lo que CABINA ensena y vuelca: cockpit, vigilancias y caja negra
//! [conecta] cabina, core, dev, fsys, mm, plat, task, uconsole
//!
//! [carril]  AMARILLO  presentacion y volcado: lo que se ve a las 3 de la manana
//! [consumo] NADA      pinta o vuelca cuando `core` lo llama
//!
//! ## Por que se partio CABINA (L8b, 2026-09-13)
//!
//! Eddi pidio desanudar el kernel empezando por CABINA, y la medida dijo por que
//! era el primer nudo: hacia DOS oficios con un solo nombre.
//!
//! ```text
//!    el REGISTRO   info/warn/fault, el anillo, la caida   lo llaman los 11
//!                                                          subsistemas
//!    el MIRADOR    cockpit, vigilancias, caja negra       lee dev, mm, plat,
//!                                                          task, fsys...
//! ```
//!
//! Juntos, la familia que todo el kernel llama tenia que importar a todo el
//! kernel: **54 usos que subian desde el suelo**. Separados, el registro queda
//! abajo --solo sabe la hora, que le da `reloj`-- y el mirador arriba, donde leer
//! a todos es precisamente su oficio.
//!
//! [!] Lo que queda, dicho: el mirador pinta con el tablero de `core::splash` y
//! lee la generacion de pantalla de `core::shell`, y `core` lo llama a el. Esa
//! arista sube, esta en la linea base de L8b, y es el siguiente corte.

use cabina_core::{Event, Layer, Severity, TelemetrySnapshot};
use crate::ring0::cabina::*;
use crate::ring0::core::splash::{splash_dashboard_log_color, DASH_LOG_W};

/// THE BLACK BOX: the ring, on the disk. The only part that survives a power
/// cut, and the only one that can fail for reasons unrelated to logging.
pub(crate) mod blackbox;
pub use blackbox::*;
/// THE WATCHES: this IS polling, and the reason is that a device which stops
/// answering does not send an event saying so.
pub(crate) mod watch;
pub(crate) use watch::*;
/// THE COCKPIT: severity colours, filters, layout. Presentation only -- none of
/// it changes what is recorded.
pub(crate) mod cockpit;
pub use cockpit::*;

/// **Volcar lo que la RAM conservo del arranque anterior a `CAIDA.TXT`**, cuando
/// el disco ya esta montado.
///
/// Devuelve los bytes escritos. Cero si no habia nada, o si el disco dijo que no
/// -- y en ese caso CABINA ya lo conto. Vivia en `cabina::caida` como `volcar`:
/// el registro entrega los bytes (`texto_recuperado`) y quien escribe en el
/// disco es quien puede importar el disco.
pub fn volcar_caida() -> usize {
    let texto = crate::ring0::cabina::caida::texto_recuperado();
    let n = texto.len();
    if n == 0 {
        return 0;
    }
    match crate::ring0::fsys::fs::create(b"CAIDA   TXT", texto) {
        Ok(()) => {
            info("caida", "guardado en CAIDA.TXT: bytes", n as u64);
            n
        }
        Err(_) => {
            warn("caida", "el disco no acepto CAIDA.TXT; sigue en RAM", n as u64);
            0
        }
    }
}
