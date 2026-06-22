//! Language adapters for BMO (v2.0.0).
//!
//! Each language provides a `LanguageAdapter` implementation that
//! compiles the language to native x86-64 via the BMO AOT pipeline.
//! The BMO ABI is THE filter: all kernel calls go through syscalls
//! 0x100..0x1FF.
//!
//! ## v2.0.0
//!
//! - Single `LanguageAdapter` trait (was `LanguagePlugin` + `LanguageAdapter`).
//! - No VM, no bytecode, no interpreter.
//! - C/C++/Java/Python adapters are STUBS. They advertise themselves
//!   as available, but `compile_native()` returns `NotSupported` until
//!   someone writes a real frontend. This is intentional — the BMO
//!   AOT is the only working compiler right now.
//!
//! To add a new language:
//! 1. Add the variant to `Language` in `super::traits`.
//! 2. Implement `LanguageAdapter` in this module.
//! 3. Register it in `super::mod::init_plugins()`.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use super::traits::{Language, LanguageAdapter, AdapterError, MemoryModel, GcStrategy};

pub mod c;
pub mod cpp;
pub mod java;
pub mod python;

// ─── BMO adapter (always available) ─────────────────────────────────

/// BMO language adapter — the only fully-implemented adapter.
/// Wraps `crate::lang::bmo::compile_native` so the BMO
/// source language goes through the same plugin pipeline as C, C++,
/// Java, and Python.
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
        // BMO source is plain text. Heuristic: has a `fn` keyword.
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("fn ") || text.contains("let ") || text.contains("si ")
    }
    fn memory_model(&self) -> MemoryModel { MemoryModel::Ownership }
    fn gc_strategy(&self) -> GcStrategy { GcStrategy::None }
}

// ─── C, C++, Java, Python adapters (stubs) ──────────────────────────

// Re-export the language-specific adapter structs.
pub use c::CAdapter;
pub use cpp::CppAdapter;
pub use java::JavaAdapter;
pub use python::PythonAdapter;
