//! `native::upload` — entrega el shader traducido al device BareX.
//!
//! ## Estado actual
//!
//! El backend real de FastOS es `barex::graphics` (GOP/software). Este
//! módulo gestiona la tabla de handles de shader: cada vez que se sube
//! un shader, devuelve un handle único que la app guarda en su PSO.
//!
//! Cuando se conecte RDNA3 (futuro), este módulo será el punto donde
//! el IR nativa se compile a RDNA3 binary y se suba al GPU driver real.
//!
//! ## Tabla de handles
//!
//! Tabla estática de hasta `MAX_HANDLES` (256) handles. Cada handle es
//! un índice en la tabla. El shader se almacena como BLAKE3 hash + tamaño;
//! el blob completo no se guarda (lo tiene el caller en su PSO).

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::primitives::{bx_u32, bx_u64, bx_u8};
use crate::diag;
use super::ir::ShaderBlob;
// v1.2.0: bsf vive en `barex::shader::bsf` (producción).
use crate::barex::shader::bsf;

/// Máximo número de shaders cargados simultáneamente.
const MAX_HANDLES: usize = 256;

/// Tamaño máximo del blob nativo cacheado (16 KB = mismo límite que BSF).
const MAX_NATIVE_SIZE: usize = 16 * 1024;

/// Entrada de la tabla de shaders subidos.
#[derive(Clone, Copy)]
struct NativeEntry {
    /// Handle asignado (== índice en la tabla + 1; 0 = inválido).
    handle: bx_u32,
    /// BLAKE3 hash del blob nativo (32 bytes).
    blake3: [u8; 32],
    /// Tamaño del blob nativo en bytes.
    size: bx_u32,
    /// Stage del shader.
    stage: bx_u8,
    /// IR de origen.
    ir: bx_u8,
    /// Slot ocupado.
    used: bool,
}

const EMPTY_ENTRY: NativeEntry = NativeEntry {
    handle: 0,
    blake3: [0; 32],
    size: 0,
    stage: 0,
    ir: 0,
    used: false,
};

static mut NATIVE_TABLE: [NativeEntry; MAX_HANDLES] = [EMPTY_ENTRY; MAX_HANDLES];
static mut NEXT_HANDLE: bx_u32 = 1;

/// Sube un blob nativo (BSF) al device BareX y devuelve su handle.
///
/// # Argumentos
/// * `blob` - el `ShaderBlob` que se va a subir.
///
/// # Retorna
/// Un `bx_u32` con el handle del shader. El handle es estable durante
/// la vida del kernel; cuando se descargue, puede ser reasignado.
///
/// # Errores
/// * `BxError::OutOfMemory` - tabla de handles llena
/// * `BxError::BufferTooSmall` - blob demasiado grande para cachear
/// * `BxError::InvalidArgument` - bytes vacíos
pub fn upload(blob: &ShaderBlob<'_>) -> BxResult<bx_u32> {
    // ── 1. Validar entrada ─────────────────────────────────────────
    if blob.bytes.is_empty() {
        diag::warn("native::upload", "empty blob");
        return Err(BxError::InvalidArgument);
    }
    if blob.bytes.len() > MAX_NATIVE_SIZE {
        diag::warn("native::upload", "blob too large");
        return Err(BxError::BufferTooSmall);
    }

    // ── 2. Calcular BLAKE3 del blob ────────────────────────────────
    let hash = crate::bef::blake3::hash(blob.bytes);

    // ── 3. Buscar slot libre (o uno con el mismo hash = dedup) ─────
    unsafe {
        // Primero buscar dedup
        for entry in NATIVE_TABLE.iter() {
            if entry.used && entry.blake3 == hash {
                diag::info_u64("native::upload", "dedup hit; handle=", entry.handle as u64);
                return Ok(entry.handle);
            }
        }

        // Buscar slot libre
        let mut slot_idx: Option<usize> = None;
        for (i, entry) in NATIVE_TABLE.iter().enumerate() {
            if !entry.used {
                slot_idx = Some(i);
                break;
            }
        }
        let idx = match slot_idx {
            Some(i) => i,
            None => {
                diag::warn("native::upload", "handle table full");
                return Err(BxError::OutOfMemory);
            }
        };

        // Asignar handle
        let handle = NEXT_HANDLE;
        NEXT_HANDLE = NEXT_HANDLE.wrapping_add(1);
        if NEXT_HANDLE == 0 { NEXT_HANDLE = 1; }  // saltar 0 = inválido

        NATIVE_TABLE[idx] = NativeEntry {
            handle,
            blake3: hash,
            size: blob.bytes.len() as bx_u32,
            stage: blob.stage.raw(),
            ir: blob.ir.raw(),
            used: true,
        };

        diag::info_u64("native::upload", "uploaded handle=", handle as u64);
        Ok(handle)
    }
}

/// Lookup: dado un handle, devuelve el BLAKE3 del blob asociado.
pub fn lookup_hash(handle: bx_u32) -> Option<[u8; 32]> {
    unsafe {
        for entry in NATIVE_TABLE.iter() {
            if entry.used && entry.handle == handle {
                return Some(entry.blake3);
            }
        }
        None
    }
}

/// Descarga un shader por handle. Devuelve el handle a la pool.
pub fn free(handle: bx_u32) -> BxResult<()> {
    unsafe {
        for entry in NATIVE_TABLE.iter_mut() {
            if entry.used && entry.handle == handle {
                entry.used = false;
                diag::info_u64("native::upload", "freed handle=", handle as u64);
                return Ok(());
            }
        }
        Err(BxError::NotFound)
    }
}

/// Cuenta shaders cargados (para diagnóstico).
pub fn count() -> bx_u32 {
    unsafe {
        NATIVE_TABLE.iter().filter(|e| e.used).count() as bx_u32
    }
}

/// Handler de hash BLAKE3 (re-export del módulo BSF para coherencia).
pub use bsf::compute_hash as hash_blake3;
