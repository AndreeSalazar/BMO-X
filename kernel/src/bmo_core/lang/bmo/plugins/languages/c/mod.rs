//! C language adapter for BMO (v2.0.0).
//!
//! C source code can call the BMO ABI directly. The adapter is a
//! thin wrapper that uses the BMO AOT compiler's `Expr::Call`
//! resolution — any call to a BMO ABI function (e.g. `fs_open`,
//! `win_create`) is automatically emitted as a syscall to the
//! corresponding BMO ABI number (0x100..0x1FF).
//!
//! v2.0.0: This adapter is a STUB. It advertises the C language as
//! available but does not provide a full C frontend. The actual C
//! source is currently routed through the BMO AOT, which works
//! for any C-like syntax that BMO can parse (BMO has Rust-like
//! syntax, not C). A real C frontend would parse C, translate to
//! BMO AST, and then call the AOT.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use super::super::traits::{Language, LanguageAdapter, AdapterError, MemoryModel, GcStrategy};

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod translator;
pub use translator::compile_c_to_native;

/// C language adapter.
pub struct CAdapter;

impl CAdapter {
    pub const fn new() -> Self { Self }
}

impl LanguageAdapter for CAdapter {
    fn language(&self) -> Language { Language::C }
    fn extensions(&self) -> &[&'static str] { &["c", "h"] }
    fn compile_native(&self, source: &[u8]) -> Result<Vec<u8>, AdapterError> {
        // Route through the C translator.
        // The translator currently produces BMO AST and then AOTs it.
        compile_c_to_native(source).map_err(|_| AdapterError::SyntaxError)
    }
    fn can_compile(&self, source: &[u8]) -> bool {
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("#include")
            || text.contains("int main")
            || text.contains("void ")
            || text.contains("printf")
    }
    fn memory_model(&self) -> MemoryModel { MemoryModel::Manual }
    fn gc_strategy(&self) -> GcStrategy { GcStrategy::None }
}
