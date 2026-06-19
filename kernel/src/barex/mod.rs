//! BareX — API moderna y nativa de FastOS
//!
//! v1.2.0: **Reorganización mayor**. Solo el código que se ejecuta en
//! producción vive aquí. Los esqueletos y blueprints (definiciones
//! vacías esperando Ring 3 / GPU) se movieron a `_blueprint/`.
//!
//! ## Lo que está AQUÍ (producción)
//!
//! - `compat`     — PE thunks / WINE-style redirección (lo carga el BEF devourer)
//! - `shader::bsf`— Loader BSF con BLAKE3 (Ring 0)
//! - `shader::loader` — Dispatcher BLAKE3 + cache lookup
//!
//! ## Lo que está en `_blueprint/` (diseño, no producción)
//!
//! - `_blueprint::audio`      — `bx_audio` API (40 archivos, ~30K líneas)
//! - `_blueprint::graphics`   — `BxDevice`, `BxSwapchain`, etc. (17 archivos)
//! - `_blueprint::input`      — HID / gamepad / keyboard (39 archivos)
//! - `_blueprint::net`        — TCP/UDP/QUIC/TLS (33 archivos)
//! - `_blueprint::shader::{spirv,dxil,dxbc,ir,native,cache,stage}` — Stubs
//!
//! Esos módulos existen como **documentación ejecutable**: describen
//! la API que tendrá BareX cuando llegue Ring 3 + GPU. Compilan pero
//! cada método retorna `BxError::NotImplemented`. Ver
//! `_blueprint/README.md` para el roadmap.
//!
//! ## Estratificación
//!
//! ```text
//!   L4  compat::*   (PE loader + COM thunks DX9/10/11/12, WINE-style)
//!   L2  shader::bsf + loader (BLAKE3 + dispatcher)
//!   L1  backend     ←── ahora GOP/software; GPU real será opcional.
//! ```
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
    pub use crate::bmo_abi::*;
}

// ── Producción (v1.2.0 reorganización) ────────────────────────────────

/// PE thunks / WINE-style redirección de DLLs. Lo usa `bef::loader::pe`.
pub mod compat;

/// BSF (BareX Shader Format) loader y dispatcher.
/// Usado por Ring 0 cuando un blob BSF entra por BEF.
pub mod shader;

// ── Blueprint (diseño, no producción) ────────────────────────────────

/// Diseño de la API completa de BareX: audio, graphics, input, net,
/// shader backends. Cada método retorna `NotImplemented` — son
/// esqueletos esperando Ring 3 + GPU. Ver `_blueprint/README.md`.
pub mod _blueprint;


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
