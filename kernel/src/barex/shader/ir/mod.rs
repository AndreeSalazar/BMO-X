//! IR y blob de entrada. Reemplaza `D3D12_SHADER_BYTECODE` / `VkShaderModuleCreateInfo`.

use crate::barex::abi::primitives::bx_u8;
use super::stage::ShaderStage;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderIr {
    /// Blob nativo opcional de un backend acelerado futuro.
    NativeGpuBinary = 0,
    /// SPIR-V 1.6 — IR canónico interno (delegado a NAGA).
    SpirV16   = 1,
    /// DXIL precompilado (delegado a `vkd3d-shader-rs`).
    Dxil      = 2,
    /// DXBC legacy (delegado a `dxvk-spirv-rs`).
    Dxbc      = 3,
}

impl ShaderIr {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }

    /// True si requiere traducción antes del upload/validación.
    #[inline(always)]
    pub const fn needs_translation(self) -> bool {
        !matches!(self, Self::NativeGpuBinary)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ShaderBlob<'a> {
    pub stage: ShaderStage,
    pub ir: ShaderIr,
    pub bytes: &'a [u8],
}
