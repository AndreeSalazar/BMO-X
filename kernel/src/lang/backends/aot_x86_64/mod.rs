//! `lang::backends::aot_x86_64` — AOT compiler para x86-64.
//!
//! Es el **único backend** activo en FastOS. Compila el BMO IR
//! (`common::ast::Module`) directamente a bytes x86-64 nativos.
//!
//! ## Módulos
//!
//! - `emit`   — emite bytes x86-64 (operaciones, instrucciones, directivas)
//! - `regs`   — asignación de registros (Register Allocator)
//! - `abi`    — convención de llamada SysV AMD64 (registros, stack)
//! - `codegen` — convierte common IR a x86-64 usando los otros 3
//!
//! ## Pipeline
//!
//! ```text
//! common::ast::Module
//!   → codegen (per-function)
//!     → regs (register allocation)
//!     → emit (instruction encoding)
//!   → CompiledArtifact { code, rodata, call_patches, string_offsets, function_offsets }
//! ```
//!
//! El linker (futuro) usa `call_patches` para resolver referencias
//! cross-function y `string_offsets` para enlazar rodata.

#![allow(dead_code)]

pub mod emit;
pub mod regs;
pub mod abi;
pub mod codegen;

pub use codegen::{compile_module, CompiledArtifact};
pub use emit::Emitter;
pub use regs::{RegAlloc, Var, VarSize, Location};
