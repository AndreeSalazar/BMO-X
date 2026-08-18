//! # MAQUETA -- VERDICT, the great-grandson generation
//!
//! **The only one with an opinion.** Everything below computes; this one judges.
//!
//! It reads finished rects and repeats no arithmetic -- it looks at what the
//! grandson produced and says whether it is any good. That is the difference
//! between `contra` and `bmo-juicio` in the cycle meter, and it is why this is a
//! crate of its own: **it is the piece that will change most often.** Every new
//! rule about what counts as wrong touches this file and nothing else. The
//! arithmetic must never be touched when the policy moves.
//!
//! ## ⚠️ La comprobacion 1 del contrato no se puede fallar aqui
//!
//! `LA_MAQUETA_EXIGE.md` seccion 7 la listaba primera: *toda etiqueta, atributo
//! y propiedad estan en las listas cerradas*. **No hay forma de que falle.** No
//! existe `Tag::H1` ni `Prop::BoxShadow`, asi que un documento que no cumpla eso
//! no llega hasta aqui: muere en el padre, y por no poder ser NOMBRADO.
//!
//! Escribir la comprobacion habria dado una funcion que siempre dice que si --
//! un guardian de mentira, que es peor que ninguno porque da confianza. El
//! contrato queda corregido.
//!
//! ## Las diez que si
//!
//! ```text
//!    fit.rs      A caja fuera de su padre     B texto que no cabe  C caja de cero
//!    names.rs    D id repetido   E isla sin sitio o repetida
//!                I regla muerta  J clase huerfana
//!    idle.rs     G gap sin flex  H absoluta sin left/top  F texto sin color
//! ```
//!
//! ## Todas son errores, y no hay avisos
//!
//! Tambien `gap` sobrante y regla muerta, que no rompen ninguna imagen. Es la
//! regla que ordena el proyecto: *nada que compile y no haga lo que dice*. Una
//! linea que se acepta y no se honra es la mentira que envejece sin avisar.
//!
//! La unica excepcion tiene su razon escrita y no es una excepcion de verdad: en
//! un fichero **sin cajas** -- una paleta como `tema/tema.maqueta` -- no se juzga
//! el uso de las reglas, porque donde no hay maquetacion no hay maquetacion que
//! juzgar. Ver `names::reglas`.

#![forbid(unsafe_code)]

pub mod fit;
pub mod idle;
pub mod names;

use bmo_maqueta_cascade::Cascaded;
use bmo_maqueta_diag::Error;
use bmo_maqueta_layout::Laid;

/// Es esto una maquetacion, o solo un fragmento?
///
/// ★ Un fichero de paleta como `tema/tema.maqueta` no tiene ni una caja: solo
/// reglas para que las usen OTROS ficheros. Sobre el no hay nada que juzgar --
/// su raiz mide 0x0 y todas sus reglas salen sin usar, y las dos cosas son la
/// consecuencia trivial de no tener cajas, no defectos del fichero.
///
/// Se decide **aqui y una sola vez**. La primera version lo tenia repartido como
/// una excepcion dentro de una comprobacion, y se le escapo otra: el veredicto
/// aprobaba las reglas del tema y acto seguido se quejaba de que su raiz media
/// cero. Una excepcion suelta tapa el sintoma que se vio, no el que viene.
pub fn es_fragmento(laid: &Laid) -> bool {
    laid.root.children.is_empty()
}

/// Judge a finished layout. Empty means it is sound.
pub fn judge(laid: &Laid, cascaded: &Cascaded) -> Vec<Error> {
    if es_fragmento(laid) {
        return Vec::new();
    }
    let mut out = Vec::new();
    fit::check(laid, &mut out);
    names::check(laid, cascaded, &mut out);
    idle::check(laid, &mut out);
    out.sort_by_key(|e| (e.span.start, e.span.len));
    out
}
