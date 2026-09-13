//! **EL REALCE DE LOS TRES BOTONES** -- cerrar, minimizar, maximizar (2026-09-13).
//!
//! [consumo] NADA      no corre en reposo: solo cuando el raton se mueve (L6h)
//!
//! ## El fallo que se vio en el Ryzen
//!
//! Eddi: *"cuando mi puntero pasa por los 3, cerrar, minimizar y maximizar, esos
//! son elementos que debes solucionar"*. El realce estaba escrito DOS veces --en
//! `datos.rs` para ESTRATOS y en `ventanas.rs` para Ejecutar-- y las dos copias
//! tenian el mismo agujero:
//!
//! ```text
//!    button_at(x, y)   solo mira si el puntero cae en la franja de SUS botones
//!                      -- no si hay OTRA ventana encima
//! ```
//!
//! Con el F12 delante de Ejecutar, pasar el raton por donde quedaban los botones
//! de Ejecutar los PINTABA por encima del F12. Y las demas ventanas --CABINA,
//! Sonido, ESTRUCTURA y las apps-- no se realzaban nunca: la misma ventana,
//! cinco comportamientos.
//!
//! ## La pieza: un sitio, y el Z-order
//!
//! ```text
//!    una app debajo del puntero   se realza ELLA; ninguna del sistema
//!    si no                        solo la ventana de ARRIBA (`top_before`),
//!                                 y solo si el puntero esta sobre ella
//!    al apagar un realce          se repinta solo si esa ventana sigue arriba:
//!                                 una ventana tapada no se pinta por encima
//! ```

use bmo_userland as bmo;

use crate::desktop::{Desktop, Ventana};
use crate::scene;

pub(crate) fn actualizar(dsk: &mut Desktop, p: &bmo::Pantalla, bajo: Option<Ventana>, x: u32, y: u32) {
    let app = dsk.table.at(x, y);
    let top = dsk.win.top_before;
    // La ventana del sistema que puede realzarse: la de arriba, con el puntero
    // encima, y sin una app delante.
    let sistema = if app.is_none() && bajo == Some(top) { Some(top) } else { None };

    macro_rules! ventana {
        ($v:expr, $abierta:expr, $chrome:expr, $fondo:expr) => {{
            let abierta = $abierta;
            let ahora = if sistema == Some($v) && abierta { $chrome.button_at(x, y) } else { None };
            if ahora != $chrome.hover {
                $chrome.hover = ahora;
                if abierta && top == $v && app.is_none() && !$chrome.minimized {
                    $chrome.paint_buttons(p, $fondo);
                }
            }
        }};
    }

    ventana!(Ventana::Run, dsk.win.visible, dsk.run_box.chrome, scene::BOX_TITLE);
    ventana!(Ventana::Data, dsk.win.data_open, dsk.win.data.chrome, scene::data::DATA_TITLE_BG);
    ventana!(Ventana::Cabina, dsk.win.cabina_open, dsk.win.cabina.chrome, scene::cabina::CAB_TITLE_BG);
    ventana!(Ventana::Sound, dsk.win.sound_open, dsk.win.sound.chrome, scene::sound::SND_TITLE_BG);
    ventana!(
        Ventana::Estructura,
        dsk.win.estructura_open,
        dsk.win.estructura.chrome,
        scene::estructura::EST_TITLE_BG
    );

    // Las apps: solo la de debajo del puntero, y las demas se apagan.
    let (fichas, n) = dsk.table.fichas();
    for &i in &fichas[..n] {
        if let Some(s) = dsk.table.get_mut(i) {
            let ahora = if app == Some(i) { s.chrome.button_at(x, y) } else { None };
            if ahora != s.chrome.hover {
                s.chrome.hover = ahora;
                if !s.chrome.minimized && !s.chrome.is_fullscreen() {
                    s.chrome.paint_buttons(p, scene::BOX_TITLE);
                }
            }
        }
    }
}
