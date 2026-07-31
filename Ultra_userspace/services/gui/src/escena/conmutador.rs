//! **El conmutador de ventanas** — la ventanita de Alt+Tab.
//!
//! La política vive en `bmo_input::foco` y **se prueba allí**; aquí sólo se
//! pinta lo que esa política ya decidió. Es el mismo reparto de siempre: quien
//! decide no dibuja, y quien dibuja no decide.
//!
//! ═══ Por qué hay que enseñarlo y no basta con conmutar ═══
//!
//! Sin esta ventanita, Alt+Tab es adivinar. Con dos ventanas se aguanta; con
//! tres ya no se sabe cuántos Tabs faltan, y el resultado es pulsar de más y
//! acabar donde no querías. Enseñar **la lista y cuál está señalada** convierte
//! un atajo de memoria en uno que se mira.
//!
//! Es lo que Eddi pidió con estas palabras: *"requiere la pequeña ventana que
//! hace ver todo para facilitar qué prioridad le da, porque si no chocan"*.

use bmo_userland as bmo;

use super::*;

const CONM_FONDO: u32 = 0x0016_2032;
const CONM_BORDE: u32 = 0x0060_80A8;
const CONM_SELECC: u32 = 0x002E_4C74;

const FILA_ALTO: u32 = bmo::GLIFO_ALTO + 8;
const CONM_ANCHO: u32 = 420;

/// El nombre de cada ventana, indexado por su id.
///
/// El id es el mismo `u8` que maneja `bmo_input::foco`: ahí es un número sin
/// significado —la política no sabe qué es una ventana— y aquí se le pone
/// nombre. Cada ventana nueva es una fila más en esta tabla.
pub(crate) fn nombre(id: u8) -> &'static str {
    match id {
        0 => "Ejecutar",
        1 => "Datos (ESTRATOS)",
        _ => "?",
    }
}

/// Pinta el conmutador centrado, con la señalada resaltada.
pub(crate) fn pintar(p: &bmo::Pantalla, lista: &[u8], señalada: usize, modo: &str) {
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
        if i == señalada {
            // El resaltado va de borde a borde: una barra a media anchura se
            // lee como "hay más columnas" y no las hay.
            p.rect(x + 6, fy - 2, ancho - 12, FILA_ALTO, CONM_SELECC);
        }
        let color = if i == señalada { TEXTO } else { TEXTO_TENUE };
        let marca = if i == señalada { "> " } else { "  " };
        let nx = p.texto(x + 14, fy + 2, marca, color);
        p.texto(nx, fy + 2, nombre(v), color);
        fy += FILA_ALTO;
    }

    // El modo, abajo: sin esto no hay forma de saber por qué el foco se
    // comporta distinto de lo que esperabas.
    let mx = p.texto(x + 14, fy + 4, "modo: ", TEXTO_TENUE);
    p.texto(mx, fy + 4, modo, ACENTO);
}

/// Qué rectángulo ocupó, para poder borrarlo después.
///
/// Se calcula igual que en `pintar` y no se guarda: dos copias de una cuenta
/// divergen, pero una cuenta guardada en un sitio y usada en otro se queda
/// vieja cuando cambia el número de ventanas — que aquí pasa en cuanto se abre
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
