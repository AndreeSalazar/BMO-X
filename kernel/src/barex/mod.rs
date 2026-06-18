//! BareX — API moderna y nativa de FastOS
//!
//! Implementación de las specs en
//! `combo_Window_Extractor/MAPA de Window/02_BEF_Format/`.
//!
//! ## Estratificación
//!
//! ```text
//!   L4  compat::*   (PE loader + COM thunks DX9/10/11/12, WINE-style)
//!   L3  graphics    (BareX_API_Spec.md — 12 objetos núcleo)
//!   L3  audio       (BareX_Audio_Spec.md — bx_audio)
//!   L3  input       (BareX_Input_Spec.md — bx_input)
//!   L3  net         (BareX_Network_Spec.md — bx_net)
//!   L2  shader      (BareX_Shader_Pipeline.md — DXIL/DXBC/SPIR-V → IR nativa)
//!   L1  backend     ←── ahora GOP/software; GPU real será opcional.
//! ```
//!
//! `barex::graphics` debe funcionar primero sobre framebuffer GOP/software. Los
//! backends acelerados se conectarán después como plugins/driver opcional, sin
//! contaminar el boot path.
//!
//! ## BMO ABI
//!
//! El **BMO ABI** (tipos primitivos, calling convention, handle, status,
//! string, time, type system, vtable, etc.) **ya no vive aquí**. Ahora está
//! en `crate::bmo_abi` como módulo top-level. Este `barex` lo usa como
//! fundación. Por compatibilidad, `crate::barex::abi` re-exporta todo.

//! ## Migración
//!
//! | Antes                          | Ahora                              |
//! |--------------------------------|------------------------------------|
//! | `crate::barex::abi::*`         | `crate::bmo_abi::*` (recomendado) |
//! | `crate::barex::abi::primitives`| `crate::bmo_abi::primitives`      |
//! | `crate::barex::abi::status`    | `crate::bmo_abi::status`          |
//! | `crate::barex::abi::handle`    | `crate::bmo_abi::handle`          |
//! | `crate::barex::abi::runtime`   | `crate::bmo_abi::runtime`         |
//!
//! El re-export `crate::barex::abi` se mantiene por 1 release, marcado deprecated.

#![allow(dead_code)]

// El BMO ABI vive en top-level. Re-exportamos por compatibilidad.
#[deprecated(since = "0.9.0", note = "Use crate::bmo_abi instead")]
pub mod abi {
    pub use crate::bmo_abi::*;
}

pub mod graphics;
pub mod audio;
pub mod input;
pub mod net;
pub mod shader;
pub mod compat;

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
