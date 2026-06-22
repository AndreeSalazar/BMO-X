//! Java language adapter for BMO (v2.0.0).
//!
//! v2.0.0: stub. The full Java frontend (with classes, interfaces,
//! vtables, exceptions) is planned for a future session. For now,
//! Java source falls back to the BMO AOT directly.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use super::super::traits::{Language, LanguageAdapter, AdapterError, MemoryModel, GcStrategy};

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod translator;
pub mod vtable;
pub mod exceptions;

/// Java language adapter.
pub struct JavaAdapter;

impl JavaAdapter {
    pub const fn new() -> Self { Self }
}

impl LanguageAdapter for JavaAdapter {
    fn language(&self) -> Language { Language::Java }
    fn extensions(&self) -> &[&'static str] { &["java"] }
    fn compile_native(&self, source: &[u8]) -> Result<Vec<u8>, AdapterError> {
        // v2.0.0: stub — direct AOT fallback.
        crate::bmo_core::lang::bmo::compile_native(source)
            .map_err(|_| AdapterError::SyntaxError)
    }
    fn can_compile(&self, source: &[u8]) -> bool {
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("public class ") || text.contains("public static void main")
    }
    fn memory_model(&self) -> MemoryModel { MemoryModel::GarbageCollected }
    fn gc_strategy(&self) -> GcStrategy { GcStrategy::Generational }
}
