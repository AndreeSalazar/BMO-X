//! **DIRECTORIES** -- names, and what they point at.
//!
//! [carril]  VERDE     nombres, y a donde apuntan
//!
//! === Why this is a file of its own ===
//!
//! It is the smallest piece of ESTRATOS and the only one that deals in NAMES.
//! Everything else here counts in blocks and node numbers; this is the layer
//! where a human-readable string turns into one, and the only place a name
//! length or an encoding can be got wrong.

use super::*;

// -- Directorios -------------------------------------------------------------

/// Entradas que caben en un listado de una vez.
///
/// Tope honesto: sin `alloc`, el buffer es fijo. Un directorio con mas
/// entries se lista TRUNCADO y se dice -- no se calla.
pub const MAX_ENTRIES: usize = 64;
pub(crate) static mut DIR_BUF: [u8; MAX_ENTRIES * ENTRADA_LEN] = [0u8; MAX_ENTRIES * ENTRADA_LEN];

/// El nodo raiz del volumen, siguiendo superbloque -> estrato -> raiz.
pub fn raiz() -> Option<(BlockPtr, Nodo)> {
    let sb = superbloque()?;
    if sb.estrato.es_nulo() { return None; }
    let e = {
        let d = seguir(&sb.estrato, 0)?;
        es::Estrato::decode(d).ok()?
    };
    let n = nodo(&e.raiz)?;
    Some((e.raiz, n))
}

/// El estrato mas reciente.
pub fn estrato() -> Option<es::Estrato> {
    let sb = superbloque()?;
    if sb.estrato.es_nulo() { return None; }
    let d = seguir(&sb.estrato, 0)?;
    es::Estrato::decode(d).ok()
}

/// Lee las entries de un directorio a UN buffer cualquiera.
///
/// Existe separada de [`entries`] porque hay dos listados en vuelo a la vez y
/// **no pueden compartir buffer**: el de `open()`, que recorre una ruta y lo
/// pisa entero en cada tramo, y el del cursor de Ring 3, que tiene que seguir
/// siendo valido entre dos preguntas del panel. Con un solo buffer, lanzar un
/// programa mientras la ventana de Datos esta abierta le cambiaba los nombres
/// bajo los pies.
pub(crate) fn listar_en(dir: &Nodo, buf: &mut [u8]) -> Option<(usize, bool)> {
    let a = dir.attr(bmo_estratos::objects::ATTR_ENTRADAS)?;
    let cabe_todo = a.size as usize <= buf.len();
    let n = flujo(a, buf)?;
    Some((n / ENTRADA_LEN, !cabe_todo))
}

/// Lee las entries de un directorio al buffer estatico.
/// Devuelve `(cuantas, si_se_trunco)`.
pub fn entries(dir: &Nodo) -> Option<(usize, bool)> {
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(DIR_BUF) };
    listar_en(dir, buf)
}

/// La entrada numero `i` del ultimo `entries()`.
pub fn entrada(i: usize) -> Option<Entrada> {
    if i >= MAX_ENTRIES { return None; }
    let buf = unsafe { &*core::ptr::addr_of!(DIR_BUF) };
    Entrada::decode(&buf[i * ENTRADA_LEN..(i + 1) * ENTRADA_LEN]).ok()
}
