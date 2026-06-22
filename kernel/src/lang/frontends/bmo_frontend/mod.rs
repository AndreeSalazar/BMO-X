//! `lang::frontends::bmo_frontend` — BMO language frontend (lex → parse → sema).
//!
//! BMO es el lenguaje nativo de FastOS. Este frontend convierte código
//! BMO al BMO IR (`common::ast`).
//!
//! ## Pipeline
//!
//! ```text
//! BMO source → Lexer → Parser → Sema → common::ast::Module
//! ```
//!
//! v1.8.8: el lexer/parser/sema son **el código legacy** de
//! `lang::bmo::lexer/parser/sema` (re-exportados). El `adapter` convierte
//! el BMO AST al BMO IR canónico.

#![allow(dead_code)]

pub mod lexer;
pub mod parser;
pub mod sema;
pub mod adapter;

use crate::lang::common::ast::Module;
use crate::bmo_gpu::BxResult;

/// Compila BMO source al BMO IR canónico.
///
/// v1.8.8: delega al pipeline legacy de `lang::bmo::compile_native` y
/// luego baja el BMO AST al BMO IR.
pub fn compile_to_ir(source: &[u8], name: &str) -> BxResult<Module> {
    // 1. Lex + parse + sema (legacy)
    let ast = crate::lang::bmo::check(source)
        .map_err(|_| crate::bmo_gpu::BxError::InvalidArgument)?;

    // 2. BMO AST → BMO IR
    Ok(adapter::lower_to_ir(&ast, name))
}

