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
//!   L2  shader      (BareX_Shader_Pipeline.md — DXIL/DXBC/SPIR-V → SASS)
//!   L1  fastgpu     ←── NO se toca aquí. Vive en `drivers::gpu::fastgpu`.
//! ```
//!
//! `barex::graphics` consumirá eventualmente la L1 vía `crate::drivers::gpu::fastgpu`,
//! pero los módulos están desacoplados para que el bridge BMO/GSP en construcción
//! pueda evolucionar sin coordinación.

#![allow(dead_code)]

pub mod graphics;
pub mod audio;
pub mod input;
pub mod net;
pub mod shader;
pub mod compat;

/// Versión mayor.menor.patch de la API BareX expuesta a Ring 3.
pub const BAREX_VERSION: (u16, u16, u16) = (1, 0, 0);

/// Identificador de hardware target congelado (RTX 3060 GA106 + Ryzen 5 5600X).
pub const HW_TARGET: &str = "GA106+Zen3";

/// Resultado canónico para toda la superficie BareX (sin `HRESULT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BxError {
    OutOfMemory,
    InvalidArgument,
    NotInitialized,
    DeviceLost,
    NotImplemented,
    Unsupported,
    Timeout,
    IoError,
}

pub type BxResult<T> = core::result::Result<T, BxError>;
