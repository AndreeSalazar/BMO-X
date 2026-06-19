//! `barex::shader` — L2 pipeline de shaders (producción).
//!
//! v1.2.0: Solo se mantienen `bsf/` (loader con BLAKE3) y `loader/`
//! (dispatcher). El resto de submódulos (stage, ir, native, spirv, dxil,
//! dxbc, cache) están en `_blueprint::shader::*` — son esqueletos
//! esperando Ring 3 / naga / vkd3d-shader-rs.
//!
//! ## Filosofía (igual que antes)
//!
//! En kernel **NO se traduce** HLSL/DXIL/DXBC/SPIR-V a código nativo
//! de GPU. Eso lo hace `nexo-sh` en tiempo de build (Ring 3) y, en
//! runtime, los crates ya existentes (`naga` para WGSL/SPIR-V,
//! `vkd3d-shader-rs` para DXIL, `dxvk-spirv-rs` para DXBC). Aquí solo
//! viven las **firmas BMO** que reciben el blob y delegan al crate
//! adecuado — **cero re-implementación**.

#![allow(dead_code)]

pub mod bsf;
pub mod loader;

// Re-exports planos para acceso ergonómico
pub use bsf::{BsfArch, BsfError, BsfShader, BsfStage};
