//! **LO QUE UN BORRADO DESTAPO** -- apuntado para devolverlo (2026-09-13).
//!
//! [consumo] NADA      apuntar son cuatro numeros; lo lee el cierre del
//!                     fotograma y lo olvida (L6h)
//!
//! ## El fallo que salio con la foto de fondo
//!
//! Eddi abrio el cubo encima de la biblioteca (F12) y lo movio: el hueco que
//! dejaba salia con **la foto del escritorio**, y la lista de la biblioteca
//! desaparecia por donde pasaba. Treinta sitios borran con `erase_window` /
//! `erase_moved`, y los dos preguntan a `scene_color`, que solo conoce la barra,
//! Ejecutar y el fondo. Despues `uncover` devuelve los iconos y Ejecutar, y el
//! F12, CABINA, Sonido y ESTRUCTURA se quedaban borrados. El propio `uncover` lo
//! tenia escrito como "hueco conocido"; con el degradado oscuro casi no se veia.
//!
//! ## La pieza: apuntar en el sitio que borra, devolver en UNO
//!
//! ```text
//!    erase_window / erase_moved   apuntan cada rectangulo que borran
//!    paint::devolver              al cerrar el fotograma: repinta las ventanas
//!                                 del sistema que tocan algo apuntado --las de
//!                                 debajo primero, la de arriba al final-- y
//!                                 recompone las apps, que van encima
//! ```
//!
//! Treinta llamadas no tienen que acordarse de nada: la que se escriba manana
//! tambien queda cubierta, porque borrar YA es apuntar.
//!
//! Se guardan hasta OCHO rectangulos sueltos y no su envolvente: arrastrar una
//! ventana deja tiras en L, y la caja que las envuelve tocaria a la propia
//! ventana movida -- que se repintaria dos veces por evento de raton. Pasado
//! el ocho, el ultimo hueco se agranda.

use core::ptr::addr_of_mut;

const MAX: usize = 8;

static mut ZONAS: [(u32, u32, u32, u32); MAX] = [(0, 0, 0, 0); MAX];
static mut N: usize = 0;

/// Apunta que `(x, y, w, h)` volvio a ser fondo.
pub(crate) fn apuntar(x: u32, y: u32, w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }
    let (zonas, n) = unsafe { (&mut *addr_of_mut!(ZONAS), &mut *addr_of_mut!(N)) };
    if *n < MAX {
        zonas[*n] = (x, y, w, h);
        *n += 1;
        return;
    }
    let (ax, ay, aw, ah) = zonas[MAX - 1];
    let (x0, y0) = (ax.min(x), ay.min(y));
    let (x1, y1) = ((ax + aw).max(x + w), (ay + ah).max(y + h));
    zonas[MAX - 1] = (x0, y0, x1 - x0, y1 - y0);
}

/// Hay algo apuntado sin devolver?
pub(crate) fn hay() -> bool {
    unsafe { N > 0 }
}

/// `(x, y, w, h)` toca algo de lo apuntado?
pub(crate) fn toca(x: u32, y: u32, w: u32, h: u32) -> bool {
    let (zonas, n) = unsafe { (&*addr_of_mut!(ZONAS), N) };
    zonas[..n]
        .iter()
        .any(|&(zx, zy, zw, zh)| x < zx + zw && zx < x + w && y < zy + zh && zy < y + h)
}

/// Lo apuntado ya se devolvio.
pub(crate) fn olvidar() {
    unsafe { N = 0 };
}
