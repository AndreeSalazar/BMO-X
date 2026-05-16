//! Dispatcher de carga de shaders — elige el crate según `ShaderIr`.

use crate::barex::BxResult;
use crate::barex::abi::primitives::bx_u32;
use super::ir::{ShaderBlob, ShaderIr};
use super::{sass, spirv, dxil, dxbc};

/// Consume un blob (cualquier IR) y devuelve handle en el device.
///
/// Pipeline:
/// ```text
///   SassGa106 → sass::upload()                          (sin traducción)
///   SpirV16   → spirv::translate_to_sass → sass::upload (vía naga + NAK)
///   Dxil      → dxil::translate_to_spirv → spirv::...   (vía vkd3d-shader-rs)
///   Dxbc      → dxbc::translate_to_spirv → spirv::...   (vía dxvk-spirv-rs)
/// ```
pub fn load(blob: &ShaderBlob<'_>) -> BxResult<bx_u32> {
    match blob.ir {
        ShaderIr::SassGa106 => sass::upload(blob),
        ShaderIr::SpirV16 => {
            let _sass = spirv::translate_to_sass(blob.bytes)?;
            // TODO: upload del Vec<u8> resultante; por ahora el sass::upload
            // espera un ShaderBlob<'_> directo del archivo BEF.
            sass::upload(blob)
        }
        ShaderIr::Dxil => {
            let _spv = dxil::translate_to_spirv(blob.bytes)?;
            // luego SPIR-V → SASS → upload
            sass::upload(blob)
        }
        ShaderIr::Dxbc => {
            let _spv = dxbc::translate_to_spirv(blob.bytes)?;
            sass::upload(blob)
        }
    }
}
