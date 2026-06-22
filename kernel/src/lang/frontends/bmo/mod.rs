//! `lang::frontends::bmo` — BMO language frontend (lex → parse → sema).
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
//! El AOT compiler (`backends::aot_x86_64`) toma ese Module y emite
//! bytes x86-64 nativos.

#![allow(dead_code)]

pub mod lexer;
pub mod parser;
pub mod sema;
pub mod adapter;

use crate::lang::common::{Module, Diagnostics};
use crate::lang::common::diagnostics::DiagCode;
use crate::lang::common::source::Span;
use crate::bmo_gpu::BxResult;

/// Compila BMO source al BMO IR canónico.
///
/// Returns `Err` con diagnósticos si hay errores de sintaxis o sema.
pub fn compile_to_ir(source: &[u8], name: &str) -> BxResult<Module> {
    let mut diags = Diagnostics::new();

    // 1. Lexer
    let mut lex = lexer::Lexer::new(source);
    let tokens = match lex.tokenize() {
        Ok(t) => t,
        Err(e) => {
            diags.error(DiagCode::SyntaxError, e.to_string(),
                        Span::point(crate::lang::common::Pos::ZERO));
            return Err(crate::bmo_gpu::BxError::InvalidArgument);
        }
    };

    // 2. Parser
    let mut parser = parser::Parser::new(&tokens);
    let ast = match parser.parse() {
        Ok(a) => a,
        Err(e) => {
            diags.error(DiagCode::SyntaxError, e.to_string(),
                        Span::point(crate::lang::common::Pos::ZERO));
            return Err(crate::bmo_gpu::BxError::InvalidArgument);
        }
    };

    // 3. Sema
    let mut sema = sema::Sema::new();
    if let Err(e) = sema.check(&ast) {
        diags.error(DiagCode::Other, e, Span::point(crate::lang::common::Pos::ZERO));
        return Err(crate::bmo_gpu::BxError::InvalidArgument);
    }

    // 4. Convert BMO AST → common IR
    let module = adapter::lower_to_ir(&ast, name);
    Ok(module)
}
