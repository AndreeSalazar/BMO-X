//! Helpers runtime. `aloc`/`libre` delegan al BMO ABI memory.

use crate::bmo_core::barex::{BxError, BxResult};
use crate::bmo_core::bmo_abi::primitives::bx_u64;

/// Reserva memoria. Stub — futuro: `barex::abi::crate::mem::alloc_pages`.
pub fn aloc(_size: usize) -> BxResult<bx_u64> {
    Err(BxError::NotImplemented)
}

/// Libera memoria reservada con `aloc`.
pub fn libre(_ptr: bx_u64) -> BxResult<()> {
    Err(BxError::NotImplemented)
}
