//! Language adapters for BMO (v2.0.0).
//!
//! Each language provides a `LanguageAdapter` implementation that
//! compiles the language to native x86-64 via the BMO AOT pipeline.
//! The BMO ABI is THE filter: all kernel calls go through syscalls
//! 0x100..0x1FF.
//!
//! ## v2.0.0 (reducido a 2 lenguajes)
//!
//! - **BMO**: el lenguaje nativo de FastOS. Pipeline completo.
//! - **C**: frontend C → AST BMO → AOT. Funcional para Hello World.
//!
//! C++/Java/Python se eliminaron en v1.8.8 para enfocar el esfuerzo.
//! Si en el futuro quieres añadirlos:
//! 1. Add the variant to `Language` in `super::traits`.
//! 2. Implement `LanguageAdapter` in this module.
//! 3. Register it in `super::mod::init_plugins()`.
//!
//! ## Modelo
//!
//! Una app C se compila así:
//! ```text
//! C source (.c) → C frontend (lexer+parser+ast)
//!              → BMO AST (translator)
//!              → AOT x86-64 (aot.rs)
//!              → BEF bytes (linker.rs)   ← NUEVO
//!              → cargada por BEF loader
//!              → saltada a Ring 3 vía iretq
//!              → BMO CORE recibe mensajes BEFCore
//! ```

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use super::traits::{Language, LanguageAdapter, AdapterError, MemoryModel, GcStrategy};

pub mod c;

// ─── BMO adapter (always available) ─────────────────────────────────

/// BMO language adapter — el único adapter con pipeline completo.
pub struct BmoAdapter;

impl BmoAdapter {
    pub const fn new() -> Self { Self }
}

impl LanguageAdapter for BmoAdapter {
    fn language(&self) -> Language { Language::Bmo }
    fn extensions(&self) -> &[&'static str] { &["bmo"] }
    fn compile_native(&self, source: &[u8]) -> Result<Vec<u8>, AdapterError> {
        crate::lang::bmo::compile_native(source)
            .map_err(|_| AdapterError::SyntaxError)
    }
    fn can_compile(&self, source: &[u8]) -> bool {
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("fn ") || text.contains("let ") || text.contains("si ")
    }
    fn memory_model(&self) -> MemoryModel { MemoryModel::Ownership }
    fn gc_strategy(&self) -> GcStrategy { GcStrategy::None }
}

// Re-export the C adapter
pub use c::CAdapter;
