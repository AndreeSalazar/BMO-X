//! Hashing de tipos BMO. Usa BLAKE3 del módulo BEF.

use crate::barex::abi::primitives::bx_u64;
use super::descriptor::TypeId;

/// Calcula el `TypeId` canónico de un tipo a partir de su nombre fully-qualified.
///
/// Construido para que dos lenguajes que nombran el mismo tipo (ej. Rust
/// `core::option::Option<i32>` vs C++ `std::optional<int32_t>` declarado
/// con el mismo `bmo_alias`) puedan compartir descriptor.
///
/// Usa BLAKE3-256 truncado a 64 bits (los primeros 8 bytes en LE).
pub fn type_hash(canonical_name: &[u8]) -> TypeId {
    let full = crate::bef::blake3::hash(canonical_name);
    let mut id: bx_u64 = 0;
    let mut i = 0;
    while i < 8 {
        id |= (full[i] as bx_u64) << (i * 8);
        i += 1;
    }
    TypeId::from_hash(id)
}
