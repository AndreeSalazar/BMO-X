//! Helpers runtime. `aloc`/`libre` delegan al BMO ABI memory.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::bx_u64;

/// Reserva memoria. Stub — futuro: `barex::abi::memory::alloc_pages`.
pub fn aloc(_size: usize) -> BxResult<bx_u64> {
    Err(BxError::NotImplemented)
}

/// Libera memoria reservada con `aloc`.
pub fn libre(_ptr: bx_u64) -> BxResult<()> {
    Err(BxError::NotImplemented)
}
