//! Python language adapter for BMO (v2.0.0).
//!
//! v2.0.0: stub. The full Python frontend (with comprehensions,
//! lambdas, classes) is planned for a future session. For now,
//! Python source falls back to the BMO AOT directly.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use super::super::traits::{Language, LanguageAdapter, AdapterError, MemoryModel, GcStrategy};

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod translator;
pub mod builtins;

/// Python language adapter.
pub struct PythonAdapter;

impl PythonAdapter {
    pub const fn new() -> Self { Self }
}

impl LanguageAdapter for PythonAdapter {
    fn language(&self) -> Language { Language::Python }
    fn extensions(&self) -> &[&'static str] { &["py"] }
    fn compile_native(&self, source: &[u8]) -> Result<Vec<u8>, AdapterError> {
        // v2.0.0: stub — direct AOT fallback.
        crate::lang::bmo::compile_native(source)
            .map_err(|_| AdapterError::SyntaxError)
    }
    fn can_compile(&self, source: &[u8]) -> bool {
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("def ") || text.contains("import ") || text.contains("print(")
    }
    fn memory_model(&self) -> MemoryModel { MemoryModel::ReferenceCounted }
    fn gc_strategy(&self) -> GcStrategy { GcStrategy::ReferenceCounting }
}
