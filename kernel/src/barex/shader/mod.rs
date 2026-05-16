//! `barex::shader` — L2 pipeline de shaders.
//!
//! Spec: `BareX_Shader_Pipeline.md`.
//!
//! ## Filosofía
//!
//! En kernel **NO se traduce** HLSL/DXIL/DXBC/SPIR-V a SASS. Eso lo hace
//! `barexc` en tiempo de build (Ring 3) y, en runtime, los crates ya
//! existentes (`naga` para WGSL/SPIR-V, `vkd3d-shader-rs` para DXIL,
//! `dxvk-spirv-rs` para DXBC). Aquí solo viven las **firmas BMO** que
//! reciben el blob y delegan al crate adecuado — **cero re-implementación**.
//!
//! Frontends esperados:
//!   - HLSL/DXIL → `dxc` offline → `vkd3d-shader-rs` runtime → SPIR-V → NAK
//!   - HLSL5/DXBC → `dxvk-spirv-rs` → SPIR-V
//!   - GLSL → `glslang` offline → SPIR-V
//!   - WGSL → `naga` → SPIR-V o directo a SASS
//!   - Slang → `slang` offline → SPIR-V
//!
//! Backend único: SPIR-V 1.6 → NAK → SASS sm_86 (GA106).
//!
//! ## Estructura modular (Sesión 14) — minimalista, una carpeta por concern
//!
//! ```
//!   shader/
//!   ├── mod.rs       ← este archivo (re-exports)
//!   ├── stage/       ← ShaderStage (12 stages)
//!   ├── ir/          ← ShaderIr + ShaderBlob (formato de entrada)
//!   ├── sass/        ← SASS GA106 nativo (upload directo al GSP)
//!   ├── spirv/       ← SPIR-V 1.6 → NAGA → SASS
//!   ├── dxil/        ← DXIL → vkd3d-shader-rs → SPIR-V
//!   ├── dxbc/        ← DXBC → dxvk-spirv-rs → SPIR-V
//!   ├── loader/      ← load() dispatcher (delega por IR)
//!   └── cache/       ← LRU de blobs ya traducidos
//! ```

#![allow(dead_code)]

pub mod stage;
pub mod ir;
pub mod sass;
pub mod spirv;
pub mod dxil;
pub mod dxbc;
pub mod loader;
pub mod cache;

// ─── Re-exports planos ───────────────────────────────────────────────
pub use stage::ShaderStage;
pub use ir::{ShaderIr, ShaderBlob};
pub use loader::load;
pub use cache::ShaderCache;
