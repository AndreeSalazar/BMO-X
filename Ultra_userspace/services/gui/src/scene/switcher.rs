//! **El conmutador de ventanas** -- la ventanita de Alt+Tab.
//!
//! La politica vive en `bmo_input::focus` y **se prueba alli**; aqui solo se
//! pinta lo que esa politica ya decidio. Es el mismo reparto de siempre: quien
//! decide no dibuja, y quien dibuja no decide.
//!
//! === Por que hay que ensenarlo y no basta con conmutar ===
//!
//! Sin esta ventanita, Alt+Tab es adivinar. Con dos ventanas se aguanta; con
//! tres ya no se sabe cuantos Tabs faltan, y el resultado es pulsar de mas y
//! acabar donde no querias. Ensenar **la lista y cual esta senalada** convierte
//! un atajo de memoria en uno que se mira.
//!
//! Es lo que Eddi pidio con estas palabras: *"requiere la pequena ventana que
//! hace ver todo para facilitar que prioridad le da, porque si no chocan"*.

use bmo_userland as bmo;

use super::*;

const SW_BG: u32 = 0x0016_2032;
const SW_EDGE: u32 = 0x0060_80A8;
const SW_SEL: u32 = 0x002E_4C74;

const ROW_H: u32 = bmo::GLIFO_ALTO + 8;
const SW_W: u32 = 420;

/// El nombre de cada ventana, indexado por su id.
///
/// El id es el mismo `u8` que maneja `bmo_input::focus`: ahi es un numero sin
/// significado --la politica no sabe que es una ventana-- y aqui se le pone
/// nombre. Cada ventana nueva es una fila mas en esta tabla.
/// * CABINA y Sonido faltaban aqui, y el atajo las ensenaba como `?`.
///
/// Las dos nacieron despues de esta tabla, y la linea de arriba --*"cada ventana
/// nueva es una fila mas"*-- no se leyo ninguna de las dos veces. El sintoma es
/// suave y por eso duro: Alt+Tab funcionaba, conmutaba bien, y solo mentia en el
/// nombre. **Una tabla que hay que acordarse de ampliar se queda corta**; lo que
/// impide que vuelva a pasar es que ahora se nombra la ventana que se esta
/// moviendo con las flechas, y un `?` que se mueve se ve enseguida.
pub(crate) fn name(id: u8) -> &'static str {
    match id {
        0 => "Ejecutar",
        1 => "Datos (ESTRATOS)",
        2 => "CABINA (kernel)",
        3 => "Sonido",
        _ => "?",
    }
}

/// El rectangulo del conmutador. **Una sola cuenta**, porque la usan dos.
///
/// `paint` y `area` la tenian copiada, con un comentario que advertia justo de
/// esto. Mientras las dos copias fueran identicas daba igual; en cuanto una
/// crece --la fila de la ayuda de las flechas-- la otra borra un rectangulo mas
/// corto que el pintado y deja una franja de la ventanita pegada en el
/// escritorio hasta el siguiente repintado.
fn run_box(p: &bmo::Pantalla, count: usize) -> (u32, u32, u32, u32) {
    // Dos filas ademas de la lista: el modo y la ayuda de las flechas.
    let height = ROW_H * count as u32 + ROW_H * 2 + 16;
    let width = SW_W.min(p.ancho.saturating_sub(40));
    (
        (p.ancho.saturating_sub(width)) / 2,
        (p.alto.saturating_sub(height)) / 2,
        width,
        height,
    )
}

/// Pinta el conmutador centrado, con la senalada resaltada.
pub(crate) fn paint(p: &bmo::Pantalla, lista: &[u8], pointed_at: usize, modo: &str) {
    if lista.is_empty() {
        return;
    }
    let (x, y, width, height) = run_box(p, lista.len());

    p.rect(x, y, width, height, SW_EDGE);
    p.rect(x + 2, y + 2, width - 4, height - 4, SW_BG);

    let mut fy = y + 10;
    for (i, &v) in lista.iter().enumerate() {
        if i == pointed_at {
            // El resaltado va de borde a borde: una barra a media anchura se
            // lee como "hay mas columnas" y no las hay.
            p.rect(x + 6, fy - 2, width - 12, ROW_H, SW_SEL);
        }
        let color = if i == pointed_at { INK } else { INK_DIM };
        let mark = if i == pointed_at { "> " } else { "  " };
        let nx = p.texto(x + 14, fy + 2, mark, color);
        p.texto(nx, fy + 2, name(v), color);
        fy += ROW_H;
    }

    // El modo, abajo: sin esto no hay forma de saber por que el foco se
    // comporta distinto de lo que esperabas. Y con el la tecla que lo cambia:
    // un modo que se lee pero no se toca invita a pensar que esta averiado.
    let mx = p.texto(x + 14, fy + 4, "modo: ", INK_DIM);
    let mx = p.texto(mx, fy + 4, modo, ACCENT);
    p.texto(mx, fy + 4, "   (Alt+M)", INK_DIM);
    fy += ROW_H;

    // ** Las flechas se anuncian AQUI y no en el pie de cada ventana.
    //
    // Porque este es el unico momento en que la mano ya tiene el Alt pulsado:
    // se lee la frase con el dedo puesto en la tecla que hace falta. En el pie
    // de CABINA seria una linea mas que se lee una vez y se olvida, y ademas
    // habria que repetirla en las tres ventanas -- tres sitios que actualizar
    // cuando el atajo cambie.
    let hx = p.texto(x + 14, fy + 4, "flechas: ", INK_DIM);
    let hx = p.texto(hx, fy + 4, "mover", INK);
    let hx = p.texto(hx, fy + 4, "   Shift+flechas: ", INK_DIM);
    p.texto(hx, fy + 4, "encajar", INK);
}

/// Que rectangulo ocupo, para poder borrarlo despues.
pub(crate) fn area(p: &bmo::Pantalla, count: usize) -> (u32, u32, u32, u32) {
    run_box(p, count)
}
