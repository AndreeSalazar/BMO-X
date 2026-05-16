//! DXBC (Shader Model 5.x legacy) → SPIR-V. Delega a `dxvk-spirv-rs`.

use crate::barex::{BxError, BxResult};
extern crate alloc;
use alloc::vec::Vec;

/// Traduce DXBC legacy a SPIR-V 1.6.
pub fn translate_to_spirv(_dxbc: &[u8]) -> BxResult<Vec<u8>> {
    Err(BxError::NotImplemented)
}
