//! `barex::shader` — L2 pipeline de shaders.
//!
//! Spec: `BareX_Shader_Pipeline.md`. Frontends: HLSL/DXIL (dxc),
//! HLSL5/DXBC (dxvk-spirv-rs), GLSL (glslang), WGSL (naga), Slang.
//! Backend único: SPIR-V 1.6 → NAK → SASS sm_86 (GA106).
//!
//! En kernel solo vive el **loader** de shaders SASS pre-compilados (los que
//! viajan empaquetados en `.bef`). La compilación HLSL→SASS la hace `barexc`
//! en tiempo de build (Ring 3, herramienta de host).

use crate::barex::{BxError, BxResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Pixel,
    Compute,
    Mesh,
    Amplification,
    RayGen,
    RayMiss,
    RayClosestHit,
    RayAnyHit,
    RayIntersect,
    RayCallable,
    WorkGraphNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderIr {
    /// SASS GA106 nativo, listo para cargar al GSP.
    SassGa106,
    /// SPIR-V 1.6 — IR canónico interno.
    SpirV16,
    /// DXIL precompilado, será traducido por vkd3d-shader-rs.
    Dxil,
    /// DXBC legacy, será traducido por dxvk-spirv-rs.
    Dxbc,
}

pub struct ShaderBlob<'a> {
    pub stage: ShaderStage,
    pub ir: ShaderIr,
    pub bytes: &'a [u8],
}

/// Loader: consume un blob (idealmente SASS) y registra en el dispositivo.
pub fn load(_blob: &ShaderBlob<'_>) -> BxResult<u32> {
    // TODO: si IR != SassGa106, traducir; si lo es, subir directo al GSP.
    Err(BxError::NotImplemented)
}
