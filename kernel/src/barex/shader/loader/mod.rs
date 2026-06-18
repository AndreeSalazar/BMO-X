//! Dispatcher de carga de shaders — elige el traductor según `ShaderIr`,
//! luego sube al device vía `native::upload`.
//!
//! ## Pipeline
//!
//! ```text
//!   NativeGpuBinary → native::upload (passthrough)
//!   SpirV16         → spirv::translate_to_native → native::upload
//!   Dxil            → dxil::translate_to_spirv   → spirv::translate_to_native → native::upload
//!   Dxbc            → dxbc::translate_to_spirv   → spirv::translate_to_native → native::upload
//! ```
//!
//! Cada paso es **idempotente y testeable**. Los stubs actuales validan
//! el formato por magic bytes y producen un SPIR-V mínimo. Cuando se
//! conecten `dxvk-spirv-rs`, `vkd3d-shader-rs` y `naga` (con RDNA3), el
//! resto del pipeline no cambia.

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::primitives::bx_u32;
use crate::diag;
extern crate alloc;
use alloc::vec::Vec;
use super::ir::{ShaderBlob, ShaderIr};
use super::{native, spirv, dxil, dxbc};

/// Consume un blob (cualquier IR) y devuelve handle en el device.
pub fn load(blob: &ShaderBlob<'_>) -> BxResult<bx_u32> {
    if blob.bytes.is_empty() {
        diag::warn("loader", "empty blob");
        return Err(BxError::InvalidArgument);
    }

    // ── 1. Traducir a SPIR-V según IR ─────────────────────────────
    let spirv_bytes: Vec<u8> = match blob.ir {
        ShaderIr::NativeGpuBinary => {
            // Ya es nativo: passthrough directo.
            diag::info("loader", "NativeGpuBinary passthrough");
            return native::upload(blob);
        }
        ShaderIr::SpirV16 => {
            diag::info("loader", "SpirV16 → validate → upload");
            spirv::translate_to_native(blob.bytes)?
        }
        ShaderIr::Dxil => {
            diag::info("loader", "Dxil → SPIR-V → validate → upload");
            let spv = dxil::translate_to_spirv(blob.bytes)?;
            spirv::translate_to_native(&spv)?
        }
        ShaderIr::Dxbc => {
            diag::info("loader", "Dxbc → SPIR-V → validate → upload");
            let spv = dxbc::translate_to_spirv(blob.bytes)?;
            spirv::translate_to_native(&spv)?
        }
    };

    // ── 2. Subir el SPIR-V (ya validado) al device ────────────────
    let upload_blob = ShaderBlob {
        stage: blob.stage,
        ir: ShaderIr::SpirV16,
        bytes: &spirv_bytes,
    };
    native::upload(&upload_blob)
}
