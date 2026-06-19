//! SPIR-V 1.6 → IR nativa BMO.
//!
//! Pipeline objetivo: `naga` (Rust) en Ring 3 para traducir SPIR-V al
//! formato intermedio de MBO. Naga es la librería que usa wgpu para
//! aceptar GLSL, WGSL, SPIR-V y producir backends (Vulkan, Metal, DX12,
//! GLSL, SPIR-V, WGSL).
//!
//! ## Estado actual
//!
//! Ring 0 **no traduce** shaders. Esta función hace:
//!   1. Validar magic SPIR-V (`0x07230203`)
//!   2. Validar versión SPIR-V (1.0+)
//!   3. Validar tamaño máximo (16 KB = límite de BSF)
//!   4. Validar que el bound sea razonable (< 65535)
//!   5. Devolver un IR placeholder (los mismos bytes SPIR-V por ahora)
//!
//! ## Cuando se conecte RDNA3 (futuro)
//!
//! Reemplazar el cuerpo con:
//! ```ignore
//! let module = naga::front::spv::parse_u8_slice(spirv)?;
//! let info = naga::valid::Validator::new(
//!     naga::valid::ValidationFlags::all(),
//!     naga::valid::Capabilities::all(),
//! ).validate(&module)?;
//! // emitir IR nativa propia desde `module`
//! ```
//! La interfaz pública **no cambia**.

use crate::barex::{BxError, BxResult};
use crate::diag;
extern crate alloc;
use alloc::vec::Vec;

/// Magic SPIR-V: little-endian 0x07230203.
const SPV_MAGIC: u32 = 0x0723_0203;

/// Versión SPIR-V mínima (1.0).
const SPV_MIN_VERSION_MAJOR: u8 = 1;

/// Tamaño máximo de un blob SPIR-V (16 KB = límite del BSF).
const SPV_MAX_SIZE: usize = 16384;

/// Bound máximo permitido en SPIR-V (el spec dice 0x3FFFF pero 65535 es
/// suficiente para kernels reales).
const SPV_MAX_BOUND: u32 = 0xFFFF;

/// Traduce un blob SPIR-V a IR nativa BMO.
///
/// # Argumentos
/// * `spirv` - bytes del shader SPIR-V, comenzando con magic `0x07230203`.
///
/// # Retorna
/// `Vec<u8>` con el IR nativa. Por ahora es el mismo SPIR-V (passthrough).
/// Cuando se conecte naga, será el IR de naga serializado al formato BMO.
///
/// # Errores
/// * `BxError::InvalidArgument` - magic incorrecto
/// * `BxError::Unsupported` - versión SPIR-V no soportada
/// * `BxError::BufferTooSmall` - blob truncado (debe ser múltiplo de 4)
/// * `BxError::BufferTooSmall` - blob excede SPV_MAX_SIZE
/// * `BxError::InvalidArgument` - bound fuera de rango
pub fn translate_to_native(spirv: &[u8]) -> BxResult<Vec<u8>> {
    // ── 1. Tamaño múltiplo de 4 (SPIR-V son words de 32 bits) ──────
    if spirv.len() < 4 {
        diag::warn("spv", "blob too small");
        return Err(BxError::BufferTooSmall);
    }
    if spirv.len() % 4 != 0 {
        diag::warn("spv", "blob size not multiple of 4");
        return Err(BxError::BufferTooSmall);
    }
    if spirv.len() > SPV_MAX_SIZE {
        diag::warn("spv", "blob too large");
        return Err(BxError::BufferTooSmall);
    }

    // ── 2. Validar magic ────────────────────────────────────────────
    let magic = u32::from_le_bytes([spirv[0], spirv[1], spirv[2], spirv[3]]);
    if magic != SPV_MAGIC {
        diag::warn("spv", "invalid magic");
        return Err(BxError::InvalidArgument);
    }

    // ── 3. Validar versión (offset 4, bytes [7:4] major [15:8] minor) ──
    let version_word = u32::from_le_bytes([spirv[4], spirv[5], spirv[6], spirv[7]]);
    let major = ((version_word >> 16) & 0xFF) as u8;
    if major < SPV_MIN_VERSION_MAJOR {
        diag::warn("spv", "unsupported SPIR-V major version");
        return Err(BxError::Unsupported);
    }

    // ── 4. Validar bound (offset 12, u32 LE) ────────────────────────
    if spirv.len() >= 16 {
        let bound = u32::from_le_bytes([spirv[12], spirv[13], spirv[14], spirv[15]]);
        if bound > SPV_MAX_BOUND {
            diag::warn("spv", "bound too large");
            return Err(BxError::InvalidArgument);
        }
    }

    // ── 5. BLAKE3 del SPIR-V para trazabilidad ──────────────────────
    let hash = crate::bef::blake3::hash(spirv);
    diag::info_u64("spv", "SPIR-V validated; hash[0..8]",
        u64::from_le_bytes([hash[0], hash[1], hash[2], hash[3],
                           hash[4], hash[5], hash[6], hash[7]]));

    // ── 6. Passthrough por ahora ────────────────────────────────────
    //
    // Cuando se conecte naga, aquí va:
    //   1. naga::front::spv::parse_u8_slice
    //   2. naga::valid::Validator::validate
    //   3. emit IR propia
    //
    // Por ahora devolvemos el mismo SPIR-V — los stubs DXBC/DXIL ya
    // producen un SPIR-V mínimo válido.
    Ok(spirv.to_vec())
}
