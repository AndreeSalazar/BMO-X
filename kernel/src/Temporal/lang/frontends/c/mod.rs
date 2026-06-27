//! `lang::frontends::c` — C language frontend.
//!
//! v1.8.8: pipeline completo. C → BMO IR canónico.
//!
//! ## Pipeline
//!
//! ```text
//! C source
//!   → preprocessor (#include, #define, #if, #ifdef)
//!   → C lexer (tokens específicos C)
//!   → C parser (recursive descent → C AST)
//!   → C → BMO translator (C AST → BMO AST legacy)
//!   → BMO AST → common IR (bmo_frontend::adapter)
//!   → Module
//! ```

#![allow(dead_code)]

pub mod preprocessor;
pub mod lexer;
pub mod parser;
pub mod translator;
pub mod adapter;

use crate::lang::common::ast::Module;
use crate::bmo_gpu::{BxError, BxResult};

/// Compila C source al BMO IR canónico.
///
/// v1.8.8: usa el translator C → BMO AST legacy + adapter al common IR.
pub fn compile_to_ir(source: &[u8], name: &str) -> BxResult<Module> {
    // 1. Preprocessor
    let pre = preprocessor::preprocess(source, name)
        .map_err(|_| BxError::InvalidArgument)?;

    // 2. C lexer
    let mut lex = lexer::CLexer::new(pre.output.as_bytes());
    let tokens = lex.tokenize()
        .map_err(|_| BxError::InvalidArgument)?;

    // 3. C parser
    let mut parser = parser::CParser::new(tokens);
    let c_ast = parser.parse()
        .map_err(|_| BxError::InvalidArgument)?;

    // 4. C AST → BMO AST
    let bmo_ast = translator::CToNexo.translate(&c_ast)
        .map_err(|_| BxError::InvalidArgument)?;

    // 5. BMO AST → common IR
    Ok(adapter::lower_bmo_ast_to_ir(&bmo_ast, name))
}
