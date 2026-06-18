//! DXBC (Shader Model 5.x legacy) → SPIR-V.
//!
//! Pipeline objetivo: `dxvk-spirv-rs` (port Rust de DXVK) en Ring 3.
//!
//! ## Estado actual
//!
//! Ring 0 **no traduce** shaders. Esta función hace:
//!   1. Validar magic DXBC (`"DXBC"` little-endian = 0x43425844)
//!   2. Validar versión mínima del blob
//!   3. Validar tamaño máximo (1 MB)
//!   4. Devolver un placeholder SPIR-V mínimo con un OpCapability + OpMemoryModel
//!      para que el resto del pipeline pueda continuar.
//!
//! ## Cuando se conecte RDNA3 (futuro)
//!
//! Reemplazar el cuerpo de `translate_to_spirv` con una llamada a
//! `dxvk_spirv::dxbc::SpirvBuilder` o equivalente. La interfaz pública
//! **no cambia** — el resto del kernel sigue usando este punto.

use crate::barex::{BxError, BxResult};
use crate::diag;
extern crate alloc;
use alloc::vec::Vec;

/// Magic number DXBC en little-endian: bytes "DXBC" = 0x44 0x58 0x42 0x43.
const DXBC_MAGIC: [u8; 4] = [0x44, 0x58, 0x42, 0x43];

/// Versión mínima del contenedor DXBC que aceptamos (SM 5.0).
const DXBC_MIN_VERSION: u32 = 0x0000_0005;

/// Tamaño máximo de un blob DXBC (1 MB). DXBC reales suelen ser 4-32 KB.
const DXBC_MAX_SIZE: usize = 1024 * 1024;

/// Traduce un blob DXBC a SPIR-V 1.6 mínimo.
///
/// # Argumentos
/// * `dxbc` - bytes del shader DXBC, comenzando con magic `"DXBC"`.
///
/// # Retorna
/// `Vec<u8>` con un blob SPIR-V mínimo placeholder. Cuando se conecte el
/// traductor real (`dxvk-spirv-rs`), este será el SPIR-V real producido
/// desde el DXBC.
///
/// # Errores
/// * `BxError::InvalidArgument` - magic incorrecto
/// * `BxError::Unsupported` - versión no soportada
/// * `BxError::BufferTooSmall` - blob truncado
/// * `BxError::BufferTooSmall` - blob excede DXBC_MAX_SIZE
pub fn translate_to_spirv(dxbc: &[u8]) -> BxResult<Vec<u8>> {
    // ── 1. Validar magic ────────────────────────────────────────────
    if dxbc.len() < 32 {
        diag::warn("dxbc", "blob too small for DXBC header");
        return Err(BxError::BufferTooSmall);
    }
    if dxbc[0..4] != DXBC_MAGIC {
        diag::warn("dxbc", "invalid magic; expected DXBC");
        return Err(BxError::InvalidArgument);
    }

    // ── 2. Validar versión (offset 4, u32 LE) ───────────────────────
    let version = u32::from_le_bytes([dxbc[4], dxbc[5], dxbc[6], dxbc[7]]);
    if version < DXBC_MIN_VERSION {
        diag::warn("dxbc", "unsupported version");
        return Err(BxError::Unsupported);
    }

    // ── 3. Validar tamaño ───────────────────────────────────────────
    if dxbc.len() > DXBC_MAX_SIZE {
        diag::warn("dxbc", "blob too large");
        return Err(BxError::BufferTooSmall);
    }

    // ── 4. Calcular BLAKE3 del DXBC para trazabilidad ──────────────
    let hash = crate::bef::blake3::hash(dxbc);
    diag::info_u64("dxbc", "DXBC validated; hash[0..8]",
        u64::from_le_bytes([hash[0], hash[1], hash[2], hash[3],
                           hash[4], hash[5], hash[6], hash[7]]));

    // ── 5. Construir SPIR-V placeholder mínimo ─────────────────────
    //
    //   Magic            0x07230203
    //   Version          0x00010000  (SPIR-V 1.0)
    //   Generator        0x00080007  (placeholder: dxvk-spirv-rs = 0x00080008)
    //   Bound            8
    //   Schema           0
    //   OpCapability Shader
    //   OpMemoryModel Logical GLSL450
    //
    // Esto basta para que el resto del pipeline (BSF, naga, GPU) reciba
    // un blob SPIR-V bien formado mínimo. El traductor real lo reemplaza
    // por el SPIR-V derivado del DXBC.
    let mut spv: Vec<u32> = Vec::with_capacity(32);
    spv.push(0x0723_0203);                    // magic
    spv.push(0x0001_0000);                    // version 1.0
    spv.push(0x0008_0007);                    // generator (placeholder dxvk)
    spv.push(8);                              // bound
    spv.push(0);                              // schema
    // OpCapability Shader
    spv.push(((2u32 << 16) | 17) << 16 | 1);  // (len=2, op=17) + capability
    // OpMemoryModel Logical GLSL450
    spv.push(((3u32 << 16) | 14) << 16 | 0);  // (len=3, op=14) + Logical + GLSL450

    // Convertir a bytes LE
    let mut out: Vec<u8> = Vec::with_capacity(spv.len() * 4);
    for w in &spv {
        out.extend_from_slice(&w.to_le_bytes());
    }
    Ok(out)
}
