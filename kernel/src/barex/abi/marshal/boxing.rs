//! Boxing/unboxing universal — para lenguajes managed.
//!
//! Stub: las operaciones reales necesitan integrarse con el GC del
//! lenguaje destino (ver `gc_iface` en una sesión futura).

use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::bx_u64;
use crate::barex::abi::type_system::TypeId;

/// Encajona un valor primitivo BMO en un objeto del lenguaje destino.
pub fn box_value(_value: bx_u64, _src_type: TypeId, _lang_id: u32) -> BxResult<bx_u64> {
    Err(BxError::NotImplemented)
}

/// Desencajona un objeto del lenguaje origen a primitivo BMO.
pub fn unbox_value(_obj: bx_u64, _src_lang: u32, _dst_type: TypeId) -> BxResult<bx_u64> {
    Err(BxError::NotImplemented)
}
