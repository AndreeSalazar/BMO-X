//! **LANZAR DESDE CUALQUIER SITIO** -- la orden que `_start` ejecuta (2026-09-13).
//!
//! [consumo] NADA      guarda una linea y la entrega una vez: no corre sola (L6h)
//!
//! ## Por que existe
//!
//! Lanzar un programa exige la pantalla y la entrada POR VALOR, y esas dos son de
//! `_start` (ver `keys::editor`). Hasta hoy el unico que podia pedirlo era la
//! caja de Ejecutar, y los iconos lo conseguian ESCRIBIENDO en ella e inyectando
//! un Enter -- que solo funciona con Ejecutar delante. Con la ventana de datos
//! delante ese Enter lo cogeria el explorador.
//!
//! ** La pieza: una ranura. Quien quiere lanzar deja aqui la linea (`pedir`) y
//! `keys::dispatch` la entrega a `_start` en la vuelta siguiente (`tomar`), por el
//! MISMO camino que un `run` tecleado: consola, prestamo de pantalla y vigilante.
//!
//! Una sola ranura y no una cola: dos lanzamientos pedidos en la misma vuelta
//! son un doble clic repetido, y el segundo pisa al primero.

use crate::scene::PATH_MAX;

static mut LINEA: [u8; PATH_MAX] = [0; PATH_MAX];
static mut LARGO: usize = 0;

/// **Pide lanzar** `partes` unidas por espacios: `[ruta]` o `[app, fichero]`.
/// `false` si no cabe -- y entonces no se pide nada: media linea lanzaria otra
/// cosa.
pub(crate) fn pedir(partes: &[&[u8]]) -> bool {
    let mut buf = [0u8; PATH_MAX];
    let mut k = 0usize;
    for (i, parte) in partes.iter().enumerate() {
        let hueco = if i > 0 { 1 } else { 0 };
        if k + hueco + parte.len() > PATH_MAX {
            return false;
        }
        if hueco == 1 {
            buf[k] = b' ';
            k += 1;
        }
        buf[k..k + parte.len()].copy_from_slice(parte);
        k += parte.len();
    }
    unsafe {
        LINEA = buf;
        LARGO = k;
    }
    k > 0
}

/// La linea pedida, UNA vez.
pub(crate) fn tomar() -> Option<([u8; PATH_MAX], usize)> {
    unsafe {
        if LARGO == 0 {
            return None;
        }
        let n = LARGO;
        LARGO = 0;
        Some((LINEA, n))
    }
}
