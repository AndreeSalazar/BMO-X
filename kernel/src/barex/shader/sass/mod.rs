//! SASS GA106 — backend nativo. Sin traducción: upload directo al GSP
//! (cuando el bridge BMO/GSP del usuario esté listo).

use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::bx_u32;
use super::ir::ShaderBlob;

/// Sube un blob SASS al GSP y devuelve su handle (índice en la tabla del device).
pub fn upload(_blob: &ShaderBlob<'_>) -> BxResult<bx_u32> {
    Err(BxError::NotImplemented)
}
