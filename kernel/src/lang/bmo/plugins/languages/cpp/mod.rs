//! C++ language adapter for BMO (v2.0.0).
//!
//! v2.0.0: stub. The full C++ frontend (with classes, inheritance,
//! vtables) is planned for a future session. For now, C++ source
//! is handled identically to C source — the BMO AOT compiles it.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use super::super::traits::{Language, LanguageAdapter, AdapterError, MemoryModel, GcStrategy};

pub mod ast;
pub mod lexer;
pub mod translator;

/// C++ language adapter.
pub struct CppAdapter;

impl CppAdapter {
    pub const fn new() -> Self { Self }
}

impl LanguageAdapter for CppAdapter {
    fn language(&self) -> Language { Language::Cpp }
    fn extensions(&self) -> &[&'static str] { &["cpp", "cc", "cxx", "hpp", "hxx"] }
    fn compile_native(&self, source: &[u8]) -> Result<Vec<u8>, AdapterError> {
        // v2.0.0: stub — uses the C translator as a fallback.
        // A real C++ frontend (with classes, vtables) would go here.
        super::c::compile_c_to_native(source).map_err(|_| AdapterError::SyntaxError)
    }
    fn can_compile(&self, source: &[u8]) -> bool {
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("class ") || text.contains("namespace ") || text.contains("template")
    }
    fn memory_model(&self) -> MemoryModel { MemoryModel::Manual }
    fn gc_strategy(&self) -> GcStrategy { GcStrategy::None }
}
