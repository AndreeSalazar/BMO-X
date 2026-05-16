//! DXIL → SPIR-V. Delega a `vkd3d-shader-rs` (port Rust de
//! `libvkd3d-shader`, parte de DXVK/VKD3D-Proton).

use crate::barex::{BxError, BxResult};
extern crate alloc;
use alloc::vec::Vec;

/// Traduce un blob DXIL a SPIR-V 1.6.
pub fn translate_to_spirv(_dxil: &[u8]) -> BxResult<Vec<u8>> {
    Err(BxError::NotImplemented)
}
