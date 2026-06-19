//! BareX — API moderna y nativa de FastOS
//!
//! v1.3.0: **Eliminación del blueprint**. Solo queda lo que se usa
//! en producción. El código DSP útil (eq, limiter, compressor,
//! reverb, dsp_math) se movió a `drivers::audio::dsp::*` donde
//! pertenece.
//!
//! ## Lo que está AQUÍ (producción)
//!
//! - `compat`     — PE thunks / WINE-style redirección (lo carga el BEF devourer)
//! - `shader::bsf`— Loader BSF con BLAKE3 (Ring 0)
//! - `shader::loader` — Dispatcher BLAKE3 + cache lookup
//!
//! ## Lo que se FUE (v1.3.0)
//!
//! - `_blueprint::audio` (40 archivos) — DSP útil a `drivers::audio::dsp::`
//!   - Stubs (engine, voice, mixer, codec, backend, ring, route, format)
//!     eliminados por no tener callers
//! - `_blueprint::input` (39 archivos) — todo stub
//! - `_blueprint::net` (33 archivos) — todo stub
//! - `_blueprint::graphics` (17 archivos) — todo stub
//! - `_blueprint::shader::` (excepto bsf/loader) — todo stub
//!
//! Si en el futuro se necesita código similar, se reescribe desde
//! `drivers::audio::dsp::*` (que tiene los algoritmos reales).
//!
//! ## BMO ABI
//!
//! El **BMO ABI** vive en `crate::bmo_abi` como módulo top-level.
//! Este `barex` lo usa como fundación. Por compatibilidad,
//! `crate::barex::abi` re-exporta todo (deprecated, use `bmo_abi`).

#![allow(dead_code)]

// El BMO ABI vive en top-level. Re-exportamos por compatibilidad.
#[deprecated(since = "0.9.0", note = "Use crate::bmo_abi instead")]
pub mod abi {
    
}

// ── Producción (v1.3.0) ────────────────────────────────────────────────

/// PE thunks / WINE-style redirección de DLLs. Lo usa `bef::loader::pe`.
pub mod compat;

/// BSF (BareX Shader Format) loader y dispatcher.
/// Usado por Ring 0 cuando un blob BSF entra por BEF.
pub mod shader;


/// Versión mayor.menor.patch de la API BareX expuesta a Ring 3.
pub const BAREX_VERSION: (u16, u16, u16) = (1, 0, 0);

/// Identificador de plataforma objetivo funcional: UEFI GOP + CPU x86_64.
pub const HW_TARGET: &str = "UEFI-GOP+x86_64";

/// Re-export de la versión del BMO ABI.
pub const BMO_ABI_VERSION: (u8, u8) = crate::bmo_abi::BMO_ABI_VERSION;

/// Resultado canónico para toda la superficie BareX (sin `HRESULT`).
///
/// En el BMO ABI viaja como `BmoStatus { code, flags, value }` empacado en
/// `RAX:RDX` (16 bytes), sin tocar memoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BxError {
    OutOfMemory     = 1,
    InvalidArgument = 2,
    NotInitialized  = 3,
    DeviceLost      = 4,
    NotImplemented  = 5,
    Unsupported     = 6,
    Timeout         = 7,
    IoError         = 8,
    PermissionDenied = 9,
    AlreadyExists   = 10,
    NotFound        = 11,
    BadHandle       = 12,
    BufferTooSmall  = 13,
}

pub type BxResult<T> = core::result::Result<T, BxError>;

impl BxError {
    /// Convierte a `BmoStatus` para retorno via BMO ABI (RAX:RDX).
    #[inline(always)]
    pub const fn to_status(self) -> crate::bmo_abi::status::BmoStatus {
        crate::bmo_abi::status::BmoStatus::err(self as u32)
    }
}
