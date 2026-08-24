//! **Las teclas de una app en ventana** -- el paso 2c de `PLAN_DIRECTOR.md`.
//!
//! Hasta hoy una app con superficie podia ENSENAR y no la podias TOCAR. El
//! plan lo decia sin adornos: *"los pasos 1, 2, 2b, 3, 4 y 5 hablan todos de
//! PIXELES. Ninguno manda un clic hacia dentro."*
//!
//! # Por que esto no cuesta un syscall por tecla
//!
//! El plan dejaba abierta una eleccion entre dos caminos, y al ir a construirla
//! **uno de los dos motivos ya no era cierto**:
//!
//! ```text
//!    A  por un ENDPOINT     969 ciclos por evento. Gratis para una
//!                           calculadora; el precio equivocado a 60 fps
//!    B  por un ANILLO en la memoria de la app, escrito directamente
//!                           en contra: "la pagina de la app tendria que
//!                           estar mapeada en el DIRECTOR, y eso es autoridad
//!                           nueva sobre un proceso ajeno"
//! ```
//!
//! ** Esa autoridad YA ESTA CONCEDIDA, y la concede la propia app. `loan::take`
//! mapea el bloque ofrecido con `RIGHT_READ | RIGHT_WRITE`: el DIRECTOR lleva
//! desde el paso 2 pudiendo escribir ahi, y lo unico que faltaba era un sitio
//! acordado donde dejar la tecla. Ese sitio es el BUZON, y lo declara la app en
//! su propia cabecera `BSUP` -- ver `<bmo/superficie.h>` y `scene::surface`.
//!
//! El plan decia que la eleccion entre A y B *"deja de ser arquitectonica y
//! pasa a ser un NUMERO: cuantos eventos por segundo"*. Con B ese numero deja
//! de existir.
//!
//! # ** DE QUIEN ES UNA TECLA: un ORDEN, nunca una heuristica
//!
//! Es la regla que esta casa ya escribio dos veces --la consola de ESTRATOS y
//! la calculadora-- y aqui se aplica igual. El orden, de arriba abajo:
//!
//! ```text
//!    1. las del ESCRITORIO      F1..F12 y cualquier cosa con Alt pulsado
//!    2. las de la APP con foco  todo lo demas, si declaro buzon
//!    3. nadie                   y entonces se descartan
//! ```
//!
//! [!] Y la lista del 1 es corta y CERRADA a proposito. Una app a pantalla
//! completa que se quedara tambien con Alt+Tab y con las F seria el modelo
//! viejo otra vez --el que entrega el aparato-- y de ese no se vuelve sin el
//! boton de reset. `Ctrl+Alt+ESC` no esta en la lista porque no le hace falta:
//! vive en Ring 0 y no depende de que nadie de aqui este vivo.

use bmo_userland as bmo;

use crate::desktop::{Desktop, Ventana};

/// F1. Los doce van seguidos salvo F11 y F12, que Set 1 puso aparte.
const SC_F1: u8 = 0x3B;
/// F10, y con ella acaba el tramo seguido.
const SC_F10: u8 = 0x44;
const SC_F11: u8 = 0x57;
const SC_F12: u8 = 0x58;

/// Bit 8 del evento crudo: hay evento.
const HAY: u64 = 0x100;

/// Cuantos eventos se sacan de la cola por vuelta.
///
/// El tope existe por lo mismo que el de la consola en `compose`: una racha de
/// teclas no puede quedarse con el bucle entero y congelar el cursor. Lo que no
/// se lea ahora sigue en el anillo del kernel y se lee en la vuelta siguiente,
/// que a la velocidad a la que gira este bucle es inmediatamente.
const POR_VUELTA: usize = 32;

/// Esta tecla es del ESCRITORIO y no se reenvia?
///
/// Se mira el SCANCODE y no el caracter porque son dos colas distintas: la
/// cocida la lee `gather` para la linea de Ejecutar, y esta es la cruda. No hay
/// forma de saber que caracter salio de que scancode, asi que la lista se
/// escribe una vez, aqui, y se comprueba contra el scancode.
fn del_escritorio(sc: u8, m: u8) -> bool {
    // Con Alt pulsado no hay tecla de app: Alt+Tab conmuta, Alt+flechas mueve
    // y Alt+M minimiza. Son del que reparte las ventanas, y quedarselas seria
    // que la ventana de delante decidiera si se puede salir de ella.
    if m & bmo::MOD_ALT != 0 {
        return true;
    }
    (SC_F1..=SC_F10).contains(&sc) || sc == SC_F11 || sc == SC_F12
}

/// **Vaciar la cola cruda y dejar en su buzon lo que sea de la app con foco.**
///
/// ** SE DRENA SIEMPRE, tenga foco una app o no, y esa es la unica parte de
/// esto que no es obvia. Si la cola solo se vaciara cuando hay a quien
/// entregar, una racha tecleada contra el escritorio se quedaria dentro, y la
/// app que ganara el foco despues recibiria de golpe un monton de teclas
/// viejas -- pulsaciones que el usuario dio a otra cosa, entregadas fuera de
/// tiempo. Una cola que solo se vacia a veces es peor que no tenerla.
pub(crate) fn reenviar(dsk: &mut Desktop, e: &bmo::Entrada, m: u8) {
    let destino = match dsk.win.focus.actual() {
        Some(Ventana::App(i)) => Some(i as usize),
        _ => None,
    };
    for _ in 0..POR_VUELTA {
        let ev = e.evento();
        if ev & HAY == 0 {
            break;
        }
        let Some(i) = destino else { continue };
        if del_escritorio((ev & 0xFF) as u8, m) {
            continue;
        }
        // `publicar` puede decir que no --buzon lleno, o una app que no lo
        // pidio-- y eso NO se reintenta: la tecla se pierde y ya. Ver el
        // motivo entero en `scene::surface::Surface::publicar`.
        if let Some(s) = dsk.table.get_mut(i) {
            s.publicar(ev);
        }
    }
}
