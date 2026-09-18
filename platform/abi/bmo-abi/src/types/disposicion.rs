//! La disposicion de un agregado, vista desde el ABI de x86-64.
//!
//! ** La REGLA (`Disposicion`, `DisposicionUnion`, `alinear`, `alineado_de`)
//! salio a `toolchain/lang/disposicion` (crate `bmo-disposicion`) el
//! 2026-09-18: la usan los frontends, y un frontend no depende del ABI de
//! x86-64. Se re-exporta aqui para que `bmo_abi::types::disposicion::X` siga
//! siendo el mismo camino. Una regla, en un sitio.
//!
//! Lo que SI es del ABI se queda aqui: `ranuras`, la convencion de llamada.

pub use bmo_disposicion::{alineado_de, alinear, Disposicion, DisposicionUnion};

/// **Cuantas ranuras de pila ocupa un argumento de `bytes` bytes.**
///
/// Esta es *la convencion de llamada de BMO*, y por eso vive aqui y no dentro
/// de un frontend: BMO **no pasa argumentos en registros**, los pasa por la
/// pila en ranuras de 8 bytes, derecha a izquierda. Un agregado ocupa
/// `techo(tamano/8)` ranuras.
///
/// Estaba escondida en `lang/c/codegen/agregados.rs` como `pub(super)`, y a la
/// vez **documentada como ABI** en `toolchain/lang/cpp/CPP_ABI.md`. Una regla que un
/// documento llama ABI y el arbol guarda dentro de un lenguaje es una regla
/// que el segundo lenguaje copia -- y ahi empieza la divergencia.
///
/// * Un agregado de 8 bytes o menos **tambien** ocupa una ranura entera. Podria
/// caber en un registro, pero tratarlo distinto obligaria al llamante y a la
/// funcion a ponerse de acuerdo sobre el tamano, y ese es justo el desacuerdo
/// que produce basura silenciosa. Una regla, sin casos de esquina -- al
/// contrario que la clasificacion por *eightbytes* de SysV, que existe porque
/// SysV si usa registros.
pub const fn ranuras(bytes: u32) -> u32 {
    if bytes <= 8 { 1 } else { (bytes + 7) / 8 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// * Un agregado pequeno ocupa una ranura ENTERA. Si los de 8 bytes o
    /// menos fueran por registro, el llamante y la funcion tendrian que
    /// ponerse de acuerdo sobre el tamano -- y ese desacuerdo produce basura
    /// silenciosa. Una regla, sin casos de esquina.
    #[test]
    fn un_agregado_pequeno_ocupa_una_ranura_entera() {
        assert_eq!(ranuras(1), 1);
        assert_eq!(ranuras(8), 1);
        assert_eq!(ranuras(9), 2);
        assert_eq!(ranuras(12), 2);
        assert_eq!(ranuras(16), 2);
        assert_eq!(ranuras(17), 3);
        // Un tipo de tamano cero sigue ocupando su sitio: cero ranuras
        // desalinearia todo lo que venga detras.
        assert_eq!(ranuras(0), 1);
    }
}
