//! `lang::frontends::c` — C language frontend.
//!
//! C es el segundo lenguaje soportado por FastOS. Este frontend
//! convierte código C al BMO IR (`common::ast`) y luego al AOT x86-64.
//!
//! ## Pipeline
//!
//! ```text
//! C source
//!   → Preprocessor (#include, #define, #ifdef)
//!   → Lexer
//!   → Parser (C AST)
//!   → Translator (C AST → BMO IR)
//!   → common::ast::Module
//! ```
//!
//! ## Soporte actual (v1.8.8)
//!
//! - Funciones (extern, static, inline)
//! - Variables globales y locales
//! - Tipos primitivos (int, char, short, long, float, double, void)
//! - Struct, union, enum
//! - Punteros y arrays
//! - Expresiones (binarias, unarias, calls, casts, sizeof, ternario)
//! - Statements (if/else, while, for, switch/case, return, break, continue, goto, label)
//! - Preprocessor básico (#include, #define, #if, #ifdef, #ifndef, #endif, #undef)
//!
//! **No soportado todavía**: macros complejas, _Generic, attributes,
//! variable-length arrays (VLA), computed gotos, setjmp/longjmp.

#![allow(dead_code)]

pub mod preprocessor;
pub mod lexer;
pub mod parser;
pub mod translator;
pub mod adapter;

use crate::lang::common::{Module, Diagnostics};
use crate::lang::common::diagnostics::DiagCode;
use crate::lang::common::source::Span;
use crate::bmo_gpu::BxResult;

/// Compila C source al BMO IR canónico.
///
/// Returns `Err` con diagnósticos si hay errores de preprocessor, lex,
/// parse, o sema.
pub fn compile_to_ir(source: &[u8], name: &str) -> BxResult<Module> {
    let mut diags = Diagnostics::new();

    // 1. Preprocessor
    let preprocessed = match preprocessor::preprocess(source, name) {
        Ok(p) => p,
        Err(e) => {
            diags.error(DiagCode::Other, e.to_string(), Span::ZERO);
            return Err(crate::bmo_gpu::BxError::InvalidArgument);
        }
    };

    // 2. Lexer
    let mut lex = lexer::Lexer::new(preprocessed.as_bytes());
    let tokens = match lex.tokenize() {
        Ok(t) => t,
        Err(e) => {
            diags.error(DiagCode::SyntaxError, e.to_string(), Span::ZERO);
            return Err(crate::bmo_gpu::BxError::InvalidArgument);
        }
    };

    // 3. Parser
    let mut parser = parser::Parser::new(&tokens);
    let c_ast = match parser.parse() {
        Ok(a) => a,
        Err(e) => {
            diags.error(DiagCode::SyntaxError, e.to_string(), Span::ZERO);
            return Err(crate::bmo_gpu::BxError::InvalidArgument);
        }
    };

    // 4. Translator (C AST → BMO AST)
    let translator = translator::CToNexo::new();
    let bmo_ast = match translator.translate(&c_ast) {
        Ok(a) => a,
        Err(e) => {
            diags.error(DiagCode::Other, e.to_string(), Span::ZERO);
            return Err(crate::bmo_gpu::BxError::InvalidArgument);
        }
    };

    // 5. Convert BMO AST → common IR
    let module = crate::lang::frontends::bmo::adapter::lower_to_ir(&bmo_ast, name);
    Ok(module)
}
