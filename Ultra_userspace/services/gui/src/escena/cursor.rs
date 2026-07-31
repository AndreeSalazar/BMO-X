//! El puntero del raton, dibujado en Ring 3.
//!
//! Su forma, su color y su contorno son decisiones de ASPECTO, y ninguna tiene
//! nada que hacer en Ring 0 — por eso el kernel entrega coordenadas y se aparta.

use bmo_userland as bmo;

use super::*;

// ── El cursor ───────────────────────────────────────────────────────────

pub(crate) const CUR_ANCHO: usize = 10;
pub(crate) const CUR_ALTO: usize = 16;
/// 0 = transparente, 1 = relleno, 2 = borde.
///
/// Borde oscuro alrededor del relleno claro: es lo que hace que una flecha se
/// vea igual de bien sobre un fondo claro que sobre uno oscuro. No es adorno,
/// es la razón de que todos los cursores del mundo tengan contorno.
pub(crate) const FLECHA: [[u8; CUR_ANCHO]; CUR_ALTO] = [
    [2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 2, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 2, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 1, 2, 0, 0, 0, 0, 0, 0],
    [2, 1, 1, 1, 2, 0, 0, 0, 0, 0],
    [2, 1, 1, 1, 1, 2, 0, 0, 0, 0],
    [2, 1, 1, 1, 1, 1, 2, 0, 0, 0],
    [2, 1, 1, 1, 1, 1, 1, 2, 0, 0],
    [2, 1, 1, 1, 1, 1, 1, 1, 2, 0],
    [2, 1, 1, 1, 1, 1, 2, 2, 2, 2],
    [2, 1, 1, 2, 1, 1, 2, 0, 0, 0],
    [2, 1, 2, 0, 2, 1, 1, 2, 0, 0],
    [2, 2, 0, 0, 2, 1, 1, 2, 0, 0],
    [2, 0, 0, 0, 0, 2, 1, 1, 2, 0],
    [0, 0, 0, 0, 0, 2, 1, 2, 0, 0],
    [0, 0, 0, 0, 0, 0, 2, 2, 0, 0],
];
pub(crate) const CUR_RELLENO: u32 = 0x00FF_FFFF;
pub(crate) const CUR_BORDE: u32 = 0x0000_0000;

pub(crate) fn dibujar_cursor(p: &bmo::Pantalla, x: u32, y: u32) {
    for (fila, linea) in FLECHA.iter().enumerate() {
        for (col, &v) in linea.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let color = if v == 1 { CUR_RELLENO } else { CUR_BORDE };
            p.punto(x + col as u32, y + fila as u32, color);
        }
    }
}

/// Restaura de la escena el rectángulo donde estaba el cursor. Devuelve `true`
/// si ese rectángulo tocaba la caja — y entonces hay letras que reescribir,
/// porque la escena sabe de rectángulos pero no de glifos.
pub(crate) fn borrar_cursor(p: &bmo::Pantalla, c: &Caja, visible: bool, x: u32, y: u32) -> bool {
    let mut toco = false;
    for fila in 0..CUR_ALTO as u32 {
        for col in 0..CUR_ANCHO as u32 {
            let (px, py) = (x + col, y + fila);
            if visible && c.contiene(px, py) {
                toco = true;
            }
            p.punto(px, py, color_escena(c, visible, px, py));
        }
    }
    toco
}

