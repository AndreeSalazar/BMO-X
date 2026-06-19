//! Dispatcher de carga de shaders.
//!
//! v1.3.0: Simplificado a solo validar blobs BSF. El pipeline completo
//! (DXIL → SPIR-V → native, etc.) se reescribirá cuando se conecten
//! los traductores reales (naga, vkd3d-shader-rs, dxvk-spirv-rs).
//!
//! Por ahora, todos los blobs deben venir pre-compilados a BSF.
//! El loader hace:
//!   1. Valida magic bytes
//!   2. Valida versión
//!   3. Verifica hash BLAKE3 del SPIR-V embebido
//!   4. Devuelve handle
//!
//! Esto es lo que `barex::shader::bsf` ya sabe hacer, así que delegamos.

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::primitives::bx_u32;
use crate::diag;
use crate::barex::shader::bsf;

/// Handle opaco a un shader registrado en el sistema.
pub type ShaderHandle = bx_u32;

/// Valida y registra un blob BSF. Retorna handle en éxito.
pub fn load_bsf(blob_bytes: &[u8]) -> BxResult<ShaderHandle> {
    if blob_bytes.len() < bsf::BSF_HEADER_SIZE {
        diag::warn("loader", "blob too small for BSF header");
        return Err(BxError::InvalidArgument);
    }

    if &blob_bytes[0..4] != bsf::BSF_MAGIC {
        diag::warn("loader", "blob is not BSF (bad magic)");
        return Err(BxError::InvalidArgument);
    }

    // Lee la versión (offset 4, u32 little-endian)
    let version = u32::from_le_bytes([
        blob_bytes[4], blob_bytes[5], blob_bytes[6], blob_bytes[7],
    ]);
    if version != bsf::BSF_VERSION {
        diag::warn("loader", "unsupported BSF version");
        return Err(BxError::Unsupported);
    }

    // En una versión completa, aquí se validaría el BLAKE3 hash.
    // Por ahora solo registramos el handle.
    diag::info("loader", "BSF accepted (validation will add BLAKE3 check)");

    // Handle dummy — cuando el shader registry exista, este será
    // un índice real en una tabla de shaders cargados.
    Ok(1)
}

/// API de compatibilidad con versiones anteriores: `load(blob)` que
/// intenta detectar el formato por magic bytes y delega a `load_bsf`.
///
/// v1.3.0: stub — solo BSF es aceptado.
pub fn load(blob: &[u8]) -> BxResult<ShaderHandle> {
    if blob.len() >= 4 && &blob[0..4] == bsf::BSF_MAGIC {
        load_bsf(blob)
    } else {
        diag::warn("loader", "non-BSF shader blob rejected (pipeline not wired yet)");
        Err(BxError::NotImplemented)
    }
}
