//! Blob nativo de backend acelerado futuro.
//!
//! El backend funcional actual de FastOS es GOP/software, así que este módulo
//! sólo define el punto de extensión para cuando exista un driver real.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::bx_u32;
use super::ir::ShaderBlob;

/// Sube un blob nativo opcional y devuelve su handle.
pub fn upload(_blob: &ShaderBlob<'_>) -> BxResult<bx_u32> {
    Err(BxError::NotImplemented)
}
