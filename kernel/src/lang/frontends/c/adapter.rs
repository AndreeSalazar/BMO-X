//! Adapter: C → common IR (a través de BMO AST).
//!
//! El camino completo es:
//! C source → preprocessor → C lexer → C parser → C AST → translator
//! → BMO AST → adapter (lowering) → common::ast::Module
//!
//! v1.8.8: delega a `frontends::bmo_frontend::adapter::lower_to_ir`.

#![allow(dead_code)]

use crate::lang::common::ast::Module;
use crate::lang::bmo::parser::ast::Ast;

/// Lowering: BMO AST (producido por el translator C) → common IR.
pub fn lower_bmo_ast_to_ir(ast: &Ast, name: &str) -> Module {
    crate::lang::frontends::bmo_frontend::adapter::lower_to_ir(ast, name)
}
