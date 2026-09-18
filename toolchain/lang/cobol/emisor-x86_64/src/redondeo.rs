//! La palabra de COBOL para redondear (`ROUNDED ...`) traducida a la
//! aritmetica compartida de `bmo-lower` (2026-09-18).
//!
//! Vive aparte de `codegen.rs` a proposito: ese fichero esta en la linea base de
//! L6a y solo puede ENCOGER, y partir el crate no es excusa para subirle el techo.

/// La palabra de COBOL (`ROUNDED ...`) a la aritmetica compartida de
/// `bmo-lower`. ** Hasta el 2026-09-18 eran el MISMO tipo (un alias) y el arbol
/// dependia del emisor; la traduccion es uno a uno y en el mismo orden.
pub(crate) fn modo(r: crate::ast::Redondeo) -> bmo_lower::redondeo::Modo {
    use crate::ast::Redondeo as R;
    use bmo_lower::redondeo::Modo as M;
    match r {
        R::Truncar => M::Truncar,
        R::MasCercanoLejosDeCero => M::MasCercanoLejosDeCero,
        R::MasCercanoPar => M::MasCercanoPar,
        R::MasCercanoHaciaCero => M::MasCercanoHaciaCero,
        R::HaciaArriba => M::HaciaArriba,
        R::HaciaAbajo => M::HaciaAbajo,
    }
}
