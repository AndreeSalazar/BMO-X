//! ÑEXO C Frontend — Compilador C → ÑEXO.
//!
//! Convierte un subconjunto de C en AST de ÑEXO, que luego
//! pasa por el codegen ÑEXO → BMOasm → nativo.
//!
//! ## Subconjunto de C soportado
//!
//! - Tipos: `int`, `unsigned int`, `long`, `char`, `void`, `char*`, `int*`, `struct`
//! - Control: `if`/`else`, `while`, `for`, `return`, `break`, `continue`
//! - Expresiones: literales, operadores, llamadas, asignación
//! - Declaraciones: `fn`, `static`, `extern`
//! - Compilación separada via `#include` stubs

#![allow(dead_code)]

pub mod lexer;
pub mod ast;
pub mod parser;
pub mod translator;

pub use lexer::CLexer;
pub use parser::CParser;
pub use translator::CToNexo;

use crate::barex::BxResult;
use super::parser::Ast;

/// Compile C source code to ÑEXO AST.
pub fn compile_c(source: &[u8]) -> BxResult<Ast> {
    let mut lexer = CLexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = CParser::new(tokens);
    let cast = parser.parse()?;
    let translator = CToNexo::new();
    translator.translate(&cast)
}
