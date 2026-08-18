//! **EL PANEL DE ARBOL**: la rama por la que has bajado, con sus hermanas.
//!
//! === Que contesta, que la miga de pan no ===
//!
//! La miga dice DONDE estas: `/ > datos > notas`. Lo que no dice es **que mas
//! habia** en cada tramo. Para ir de `/datos/notas` a `/datos/fotos` con solo
//! la miga hay que subir y volver a bajar; con el arbol es un clic, porque
//! `fotos` esta ahi al lado de `notas`.
//!
//! Eso es todo lo que compra un panel de arbol, y es bastante: **los hermanos
//! de cada nivel a la vez**.
//!
//! === Solo directorios, y no es una simplificacion ===
//!
//! Los archivos no salen aqui. El arbol es por donde se NAVEGA, y a un archivo
//! no se entra -- una lista de la que la mitad de las filas no hace nada al
//! pulsarlas ensena a no pulsar. Los archivos viven en la rejilla, que es donde
//! se trabaja con ellos.
//!
//! === UN recorrido para pintar y para acertar con el raton ===
//!
//! [`filas`] enumera las filas en el orden en que se ven, y la usan los dos:
//! el que pinta y el que decide sobre cual cayo el clic. Escribirlo dos veces
//! es lo que ya tiene su aviso en `box_at` -- *si una cambia y la otra no, se
//! pulsa una fila y se selecciona otra*.
//!
//! === De donde salen los datos ===
//!
//! De las cuatro preguntas por nivel que el cursor aprendio a contestar el
//! 2026-08-18 (`nivel_hijos`, `nivel_hijo_tipo`, `nivel_hijo_nombre`,
//! `nivel_elegido`). **Ninguna toca el disco**: cada nivel guarda su listado
//! desde que se paso por el. Sin eso, pintar este panel serian cientos de
//! lecturas de bloque por repintado y el arbol no se podria pintar.

use bmo_userland as bmo;

use super::zonas::Zona;
use super::{INK, INK_DIM};

/// Alto de una fila. El mismo que la rejilla, para que las dos columnas se lean
/// como una sola tabla y no como dos programas pegados.
pub(crate) const ROW_H: u32 = 22;
/// Cuanto se mete hacia dentro cada nivel.
const SANGRIA: u32 = 12;
/// Lo mas que se ensena de un nombre en este panel.
const NOMBRE_MAX: usize = 20;

/// Cuantas filas se piden de una vez. Con `ROW_H` a 22, cuarenta filas son 880
/// pixeles de alto: mas de lo que cabe en ninguna ventana de esta pantalla, asi
/// que el tope nunca recorta lo que se ve.
pub(crate) const FILAS_MAX: usize = 40;

/// Una fila del arbol.
#[derive(Clone, Copy)]
pub(crate) struct Fila {
    pub nivel: u64,
    pub indice: u64,
    /// Es la rama por la que se bajo. Se pinta abierta y en claro.
    pub abierta: bool,
}

impl Fila {
    pub const VACIA: Self = Self { nivel: 0, indice: 0, abierta: false };
}

/// **Enumera las filas del arbol en el orden en que se ven.**
///
/// Llena `dst` a partir de la fila `desde` y devuelve **cuantas hay en total**,
/// que casi nunca es cuantas caben -- por eso se devuelven las dos cosas.
pub(crate) fn filas(desde: usize, dst: &mut [Fila]) -> usize {
    let hondo = bmo::estratos::hondo();
    let mut total = 0usize;
    let mut n = 0usize;
    enumerar(0, hondo, desde, dst, &mut total, &mut n);
    total
}

/// El recorrido de verdad.
///
/// Es recursivo y esta acotado: solo se baja por la rama ABIERTA, y solo hay
/// una por nivel, asi que la profundidad es la del cursor -- dieciseis como
/// mucho, que es su `HONDO_MAX`. No es un recorrido del arbol entero: es el
/// camino, con los hermanos de cada tramo.
fn enumerar(
    nivel: u64,
    hondo: u64,
    desde: usize,
    dst: &mut [Fila],
    total: &mut usize,
    n: &mut usize,
) {
    let elegido = bmo::estratos::nivel_elegido(nivel);
    let cuantos = bmo::estratos::nivel_hijos(nivel);
    let mut i = 0u64;
    while i < cuantos {
        if bmo::estratos::nivel_hijo_tipo(nivel, i) == bmo::estratos::DIRECTORIO {
            let abierta = elegido != bmo::estratos::NINGUNO && i == elegido;
            if *total >= desde && *n < dst.len() {
                dst[*n] = Fila { nivel, indice: i, abierta };
                *n += 1;
            }
            *total += 1;
            // Los hijos de la rama abierta van JUSTO DEBAJO de ella, no al
            // final de su nivel: es lo que hace que la sangria se lea como un
            // arbol y no como tres listas apiladas.
            if abierta && nivel < hondo {
                enumerar(nivel + 1, hondo, desde, dst, total, n);
            }
        }
        i += 1;
    }
}

/// Cuantas filas caben en `z`, descontando la de la raiz.
pub(crate) fn caben(z: &Zona) -> usize {
    (z.h.saturating_sub(ROW_H) / ROW_H).max(1) as usize
}

/// **Sobre que fila cayo el puntero.**
///
/// `Some(None)` es la raiz --la primera linea, que siempre esta-- y
/// `Some(Some(f))` una fila del arbol. `None` es fuera del panel.
///
/// El doble `Option` no es coqueteria: subir a la raiz y bajar a un hijo son
/// dos gestos distintos y el que llama tiene que poder distinguirlos. Meter la
/// raiz como una fila mas obligaria a inventarle un nivel que no tiene.
pub(crate) fn fila_en(z: &Zona, desde: usize, px: u32, py: u32) -> Option<Option<Fila>> {
    if !z.contiene(px, py) {
        return None;
    }
    let k = (py - z.y) / ROW_H;
    if k == 0 {
        return Some(None);
    }
    let mut buf = [Fila::VACIA; FILAS_MAX];
    let cuantas = caben(z).min(FILAS_MAX);
    // La misma llamada da las filas Y cuantas hay en total, asi que no hace
    // falta recorrer dos veces para saber si el clic cayo en panel vacio.
    let total = filas(desde, &mut buf[..cuantas]);
    let idx = (k - 1) as usize;
    // Pulsar por debajo de la ultima fila es pulsar el panel, no la ultima.
    if idx >= cuantas || desde + idx >= total {
        return None;
    }
    Some(Some(buf[idx]))
}

/// Pinta el panel.
pub(crate) fn paint(p: &bmo::Pantalla, z: &Zona, desde: usize, accent: u32, sel_bg: u32) {
    if !z.hay() {
        return;
    }
    // -- La raiz, siempre arriba y siempre visible --
    //
    // No entra en el desplazamiento a proposito: es el unico sitio al que
    // siempre se puede volver, y una lista larga que se lleva el `/` fuera de
    // la vista deja sin salida a quien se ha perdido.
    let raiz_ink = if bmo::estratos::hondo() == 0 { accent } else { INK };
    p.texto(z.x + 4, z.y + (ROW_H - bmo::GLIFO_ALTO) / 2, "/", raiz_ink);
    p.texto(
        z.x + 4 + 2 * bmo::GLIFO_ANCHO,
        z.y + (ROW_H - bmo::GLIFO_ALTO) / 2,
        "raiz",
        raiz_ink,
    );

    let cuantas = caben(z).min(FILAS_MAX);
    let mut buf = [Fila::VACIA; FILAS_MAX];
    let total = filas(desde, &mut buf[..cuantas]);
    let vistas = cuantas.min(total.saturating_sub(desde));

    let mut y = z.y + ROW_H;
    for f in buf.iter().take(vistas) {
        let x = z.x + 4 + (f.nivel as u32 + 1) * SANGRIA;
        if f.abierta {
            // El realce ocupa el ancho del panel: es como se lee "estas dentro
            // de esta" sin un cursor parpadeando.
            p.rect(z.x, y, z.w, ROW_H, sel_bg);
        }
        let ty = y + (ROW_H - bmo::GLIFO_ALTO) / 2;
        // La flecha dice si esta abierta o cerrada, que es lo mismo que dice la
        // sangria de debajo -- pero la sangria hay que compararla con la fila
        // de al lado y la flecha se lee sola.
        p.texto(x, ty, if f.abierta { "v" } else { ">" }, INK_DIM);
        let mut nom = [0u8; 64];
        let n = bmo::estratos::nivel_hijo_nombre(f.nivel, f.indice, &mut nom);
        let corte = n.min(NOMBRE_MAX);
        let ink = if f.abierta { INK } else { INK_DIM };
        let fin = p.texto_bytes(x + 2 * bmo::GLIFO_ANCHO, ty, &nom[..corte], ink);
        // Un nombre recortado LO DICE. Sin esto, dos carpetas cuyo nombre solo
        // se diferencia por el final se ven iguales -- y en un panel estrecho
        // eso pasa mas de lo que parece.
        if corte < n {
            p.texto(fin, ty, "~", INK_DIM);
        }
        y += ROW_H;
    }

    // Y si hay mas de las que caben, se DICE.
    if desde + vistas < total {
        p.texto(z.x + 4, y + 2, "...mas abajo", INK_DIM);
    }
}

/// **Lleva el cursor a la fila `(nivel, indice)`.**
///
/// Un clic en el arbol es un salto: de `/a/b/c` a `/a/d` hay que subir dos
/// veces y bajar una. Aqui se hace con las operaciones que ya existen --`subir`
/// y `entrar`-- y no con una nueva del kernel, porque no hace falta ninguna: un
/// salto en un arbol es un camino, y el camino se anda.
///
/// El bucle termina solo: `subir` deja `hondo` estrictamente mas pequeno en
/// cada vuelta y contesta `false` en la raiz. No hay forma de que gire.
///
/// ** Y ninguna de las subidas toca el disco: cada nivel sigue leido desde que
/// se paso por el. Antes esto habria sido una relectura del directorio por cada
/// nivel que se sube -- o sea, un salto de tres niveles pagando tres listados.
pub(crate) fn saltar_a(nivel: u64, indice: u64) -> bool {
    while bmo::estratos::hondo() > nivel {
        if !bmo::estratos::subir() {
            return false;
        }
    }
    // Si el nivel pedido es MAS hondo que donde estamos, la fila ya no existe:
    // el arbol se pinto antes de que el cursor se moviera. No se inventa un
    // camino hacia abajo -- se dice que no y el siguiente repintado lo cuadra.
    if bmo::estratos::hondo() != nivel {
        return false;
    }
    bmo::estratos::entrar(indice)
}

/// Vuelve a la raiz subiendo, que no cuesta ni una lectura.
///
/// `a_la_raiz()` del cursor haria lo mismo **releyendo** el directorio raiz y
/// el detalle de sus hijos. Subir no relee nada: los niveles siguen ahi.
pub(crate) fn a_la_raiz_subiendo() {
    while bmo::estratos::subir() {}
}
