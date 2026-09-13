//! **LOS PICTOGRAMAS** -- que ES un fichero, dibujado y no escrito (2026-09-13).
//!
//! [consumo] NADA      pinta cuando quien lo llama pinta (L6h)
//!
//! La biblioteca nacio con una LETRA sobre un cuadro de color (`A`, `I`, `S`,
//! `T`). Eddi pidio la imagen 4 --un lanzador con iconos-- y una letra obliga a
//! saber que letra es cada clase; un marco con una montana no.
//!
//! Solo `rect`, como los tres botones de `chrome`: la fuente no trae estos
//! glifos y meterlos en el generador por cinco dibujos seria tocar la tabla que
//! comparten el kernel y Ring 3 (ver `userland/src/pantalla/verde.rs`).
//!
//! ```text
//!    App      una ventana con su barra
//!    Imagen   un marco, un sol y una montana
//!    Audio    una nota
//!    Texto    una hoja con renglones
//!    Otro     una hoja en blanco
//! ```
//!
//! Todo se mide en `u = lado / 8`, asi el mismo dibujo sirve a 20 px en la
//! lista y a 64 px en la ficha grande.

use bmo_userland as bmo;

use super::asociaciones::Clase;
use super::rounded_rect;

/// La tinta del dibujo: casi negro, que se lee sobre los cinco colores.
const TINTA: u32 = 0x0010_1418;

/// Un marco hueco de grosor `g`.
fn marco(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, g: u32, c: u32) {
    p.rect(x, y, w, g, c);
    p.rect(x, y + h - g, w, g, c);
    p.rect(x, y, g, h, c);
    p.rect(x + w - g, y, g, h, c);
}

/// Dibuja el pictograma de `clase` en un cuadro de `lado` px.
pub(crate) fn dibujar(p: &bmo::Pantalla, x: u32, y: u32, lado: u32, clase: Clase) {
    rounded_rect(p, x, y, lado, lado, clase.color());
    let u = (lado / 8).max(1);
    let g = (u / 2).max(1);
    let (ix, iy) = (x + 2 * u, y + 2 * u);
    let ia = lado - 4 * u;
    match clase {
        Clase::App => {
            marco(p, ix, iy, ia, ia, g, TINTA);
            p.rect(ix, iy, ia, u + g, TINTA);
        }
        Clase::Imagen => {
            marco(p, ix, iy, ia, ia, g, TINTA);
            // El sol, arriba a la derecha.
            p.rect(ix + ia - 2 * u - g, iy + u, u, u, TINTA);
            // La montana: escalones que se estrechan hacia arriba.
            let base = iy + ia - g;
            let pasos = (ia / (2 * g)).min(ia / 2);
            let alto_paso = ((ia / 2) / pasos.max(1)).max(1);
            for k in 0..pasos {
                let w = ia.saturating_sub(2 * g).saturating_sub(k * 2 * g);
                if w == 0 {
                    break;
                }
                let yy = base.saturating_sub((k + 1) * alto_paso);
                p.rect(ix + g + (ia - 2 * g - w) / 2, yy, w, alto_paso, TINTA);
            }
        }
        Clase::Audio => {
            // Plica, cabeza y bandera.
            let px = x + lado / 2 + u / 2;
            p.rect(px, iy, g.max(u / 2 + 1), ia - u, TINTA);
            p.rect(px - 2 * u, iy + ia - 2 * u, 2 * u + g.max(u / 2 + 1), 2 * u, TINTA);
            p.rect(px, iy, 2 * u, u, TINTA);
        }
        Clase::Texto | Clase::Otro => {
            let hx = ix + u / 2;
            let hw = ia - u;
            marco(p, hx, iy, hw, ia, g, TINTA);
            if clase == Clase::Texto {
                let paso = (ia / 5).max(2);
                for k in 1..4 {
                    p.rect(hx + u, iy + k * paso, hw - 2 * u, g, TINTA);
                }
            }
        }
    }
}
