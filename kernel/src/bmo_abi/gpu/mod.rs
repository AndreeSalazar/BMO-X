//! `bmo_abi::gpu` — Contratos de GPU (skeleton, RDNA4 = no implementado).
//!
//! Define los **tipos** que un programa usaría para hablar con la GPU
//! cuando RDNA4 esté implementado. **Por ahora solo son contratos**:
//! no hay syscalls de GPU activos.
//!
//! ## Estado actual (v1.8.8)
//!
//! - No hay driver de GPU en el kernel.
//! - No hay syscalls `NR_GPU_*`.
//! - El ring0 `bmo_gpu/` es un **stub** (ver `kernel/ANALISIS_BMO_CORE_LANG.md`).
//!
//! ## Roadmap
//!
//! - v1.9.0: driver RDNA4 básico (compute shaders solo).
//! - v1.10.0: graphics pipeline (shaders gráficos + output a surface).
//! - v1.11.0: video decode (VCN).

#![allow(dead_code)]

use crate::bmo_abi::fundamentals::handle::BmoHandle;
use crate::bmo_abi::surface::BmoFormat;

// ─── Shader ────────────────────────────────────────────────────────

/// Lenguaje del shader.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoShaderLang {
    /// RDNA4 ISA (binary).
    Rdna4 = 0,
    /// GLSL cross-compiled a RDNA4.
    Glsl  = 1,
    /// HLSL cross-compiled a RDNA4.
    Hlsl  = 2,
    /// WGSL cross-compiled a RDNA4.
    Wgsl  = 3,
}

/// Tipo de shader.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoShaderStage {
    Compute = 0,
    Vertex  = 1,
    Fragment = 2,
    Geometry = 3,
}

/// Handle a un shader compilado.
pub type BmoShader = BmoHandle;

// ─── Buffer ────────────────────────────────────────────────────────

/// Uso esperado del buffer (afecta dónde se ubica: VRAM vs GART).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoBufferUsage {
    /// Vertex buffer.
    Vertex   = 1 << 0,
    /// Index buffer.
    Index    = 1 << 1,
    /// Constant/uniform buffer.
    Constant = 1 << 2,
    /// Storage (read/write desde shader).
    Storage  = 1 << 3,
    /// Texture (sampler).
    Texture  = 1 << 4,
    /// Staging (CPU → GPU upload).
    Staging  = 1 << 5,
}

/// Handle a un buffer GPU.
pub type BmoBuffer = BmoHandle;

// ─── Dispatch ──────────────────────────────────────────────────────

/// Argumentos de `bmo_gpu_dispatch(shader, grid_x, grid_y, grid_z, group_x, group_y, group_z, args)`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoDispatch {
    pub shader: BmoShader,
    pub grid: [u32; 3],
    pub group: [u32; 3],
    /// Argumentos extra (bind slots, push constants). Tamaño variable.
    pub args_ptr: u64,
    pub args_len: u32,
    pub _pad: u32,
}

// ─── Texture ───────────────────────────────────────────────────────

/// Descripción de una textura 2D.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoTexture2DInfo {
    pub w: u32,
    pub h: u32,
    pub format: BmoFormat,
    pub mip_levels: u32,
    pub array_size: u32,
    pub usage: u32, // BmoBufferUsage bitflags
}

impl BmoTexture2DInfo {
    pub fn size_bytes(&self) -> u64 {
        let bpp = self.format.bytes_per_pixel() as u64;
        (self.w as u64) * (self.h as u64) * bpp * (self.array_size as u64)
    }
}
