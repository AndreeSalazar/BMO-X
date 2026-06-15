//! `barex::shader` — L2 pipeline de shaders.
//!
//! Spec: `BareX_Shader_Pipeline.md`.
//!
//! ## Filosofía
//!
//! En kernel **NO se traduce** HLSL/DXIL/DXBC/SPIR-V a código nativo de GPU. Eso lo hace
//! `barexc` en tiempo de build (Ring 3) y, en runtime, los crates ya
//! existentes (`naga` para WGSL/SPIR-V, `vkd3d-shader-rs` para DXIL,
//! `dxvk-spirv-rs` para DXBC). Aquí solo viven las **firmas BMO** que
//! reciben el blob y delegan al crate adecuado — **cero re-implementación**.
//!
//! Frontends esperados:
//!   - HLSL/DXIL → `dxc` offline → `vkd3d-shader-rs` runtime → SPIR-V
//!   - HLSL5/DXBC → `dxvk-spirv-rs` → SPIR-V
//!   - GLSL → `glslang` offline → SPIR-V
//!   - WGSL → `naga` → SPIR-V
//!   - Slang → `slang` offline → SPIR-V
//!
//! Backend inicial: SPIR-V/IR → interpretación/validación o raster GOP/software.
//! Código nativo de GPU queda para backends acelerados opcionales.
//!
//! ## Estructura modular (Sesión 14) — minimalista, una carpeta por concern
//!
//! ```
//!   shader/
//!   ├── mod.rs       ← este archivo (re-exports)
//!   ├── stage/       ← ShaderStage (12 stages)
//!   ├── ir/          ← ShaderIr + ShaderBlob (formato de entrada)
//!   ├── native/      ← blob nativo opcional de backend futuro
//!   ├── spirv/       ← SPIR-V 1.6 → NAGA/IR
//!   ├── dxil/        ← DXIL → vkd3d-shader-rs → SPIR-V
//!   ├── dxbc/        ← DXBC → dxvk-spirv-rs → SPIR-V
//!   ├── loader/      ← load() dispatcher (delega por IR)
//!   └── cache/       ← LRU de blobs ya traducidos
//! ```

#![allow(dead_code)]

pub mod stage;
pub mod ir;
pub mod native;
pub mod spirv;
pub mod dxil;
pub mod dxbc;
pub mod loader;
pub mod cache;

// ─── Re-exports planos ───────────────────────────────────────────────
