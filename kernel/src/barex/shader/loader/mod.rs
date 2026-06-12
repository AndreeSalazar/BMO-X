//! Dispatcher de carga de shaders — elige el crate según `ShaderIr`.

use crate::barex::BxResult;
use crate::barex::abi::primitives::bx_u32;
use super::ir::{ShaderBlob, ShaderIr};
use super::{native, spirv, dxil, dxbc};

/// Consume un blob (cualquier IR) y devuelve handle en el device.
///
/// Pipeline:
/// ```text
///   NativeGpuBinary → native::upload()                         (sin traducción)
///   SpirV16         → spirv::translate_to_native → native::upload
///   Dxil      → dxil::translate_to_spirv → spirv::...   (vía vkd3d-shader-rs)
///   Dxbc      → dxbc::translate_to_spirv → spirv::...   (vía dxvk-spirv-rs)
/// ```
pub fn load(blob: &ShaderBlob<'_>) -> BxResult<bx_u32> {
    match blob.ir {
        ShaderIr::NativeGpuBinary => native::upload(blob),
        ShaderIr::SpirV16 => {
            let _native = spirv::translate_to_native(blob.bytes)?;
            // TODO: upload del Vec<u8> resultante; por ahora native::upload
            // espera un ShaderBlob<'_> directo del archivo BEF.
            native::upload(blob)
        }
        ShaderIr::Dxil => {
            let _spv = dxil::translate_to_spirv(blob.bytes)?;
            // luego SPIR-V → IR/backend nativo → upload
            native::upload(blob)
        }
        ShaderIr::Dxbc => {
            let _spv = dxbc::translate_to_spirv(blob.bytes)?;
            native::upload(blob)
        }
    }
}
