//! **El conmutador de ventanas** -- la ventanita de Alt+Tab.
//!
//! La politica vive en `bmo_input::foco` y **se prueba alli**; aqui solo se
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

const CONM_FONDO: u32 = 0x0016_2032;
const CONM_BORDE: u32 = 0x0060_80A8;
const CONM_SELECC: u32 = 0x002E_4C74;

const FILA_ALTO: u32 = bmo::GLIFO_ALTO + 8;
const CONM_ANCHO: u32 = 420;

/// El nombre de cada ventana, indexado por su id.
///
/// El id es el mismo `u8` que maneja `bmo_input::foco`: ahi es un numero sin
/// significado --la politica no sabe que es una ventana-- y aqui se le pone
/// nombre. Cada ventana nueva es una fila mas en esta tabla.
pub(crate) fn name(id: u8) -> &'static str {
    match id {
        0 => "Ejecutar",
        1 => "Datos (ESTRATOS)",
        _ => "?",
    }
}

/// Pinta el conmutador centrado, con la senalada resaltada.
pub(crate) fn pintar(p: &bmo::Pantalla, lista: &[u8], pointed_at: usize, modo: &str) {
    if lista.is_empty() {
        return;
    }
    let alto = FILA_ALTO * lista.len() as u32 + FILA_ALTO + 16;
    let ancho = CONM_ANCHO.min(p.ancho.saturating_sub(40));
    let x = (p.ancho.saturating_sub(ancho)) / 2;
    let y = (p.alto.saturating_sub(alto)) / 2;

    p.rect(x, y, ancho, alto, CONM_BORDE);
    p.rect(x + 2, y + 2, ancho - 4, alto - 4, CONM_FONDO);

    let mut fy = y + 10;
    for (i, &v) in lista.iter().enumerate() {
        if i == pointed_at {
            // El resaltado va de borde a borde: una barra a media anchura se
            // lee como "hay mas columnas" y no las hay.
            p.rect(x + 6, fy - 2, ancho - 12, FILA_ALTO, CONM_SELECC);
        }
        let color = if i == pointed_at { TEXTO } else { TEXTO_TENUE };
        let marca = if i == pointed_at { "> " } else { "  " };
        let nx = p.texto(x + 14, fy + 2, marca, color);
        p.texto(nx, fy + 2, name(v), color);
        fy += FILA_ALTO;
    }

    // El modo, abajo: sin esto no hay forma de saber por que el foco se
    // comporta distinto de lo que esperabas. Y con el la tecla que lo cambia:
    // un modo que se lee pero no se toca invita a pensar que esta averiado.
    let mx = p.texto(x + 14, fy + 4, "modo: ", TEXTO_TENUE);
    let mx = p.texto(mx, fy + 4, modo, ACENTO);
    p.texto(mx, fy + 4, "   (Alt+M)", TEXTO_TENUE);
}

/// Que rectangulo ocupo, para poder borrarlo despues.
///
/// Se calcula igual que en `pintar` y no se guarda: dos copias de una cuenta
/// divergen, pero una cuenta guardada en un sitio y usada en otro se queda
/// vieja cuando cambia el numero de ventanas -- que aqui pasa en cuanto se abre
/// una.
pub(crate) fn area(p: &bmo::Pantalla, cuantas: usize) -> (u32, u32, u32, u32) {
    let alto = FILA_ALTO * cuantas as u32 + FILA_ALTO + 16;
    let ancho = CONM_ANCHO.min(p.ancho.saturating_sub(40));
    (
        (p.ancho.saturating_sub(ancho)) / 2,
        (p.alto.saturating_sub(alto)) / 2,
        ancho,
        alto,
    )
}
