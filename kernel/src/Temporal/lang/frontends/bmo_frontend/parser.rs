//! BMO Parser — convierte tokens a BMO AST.
//!
//! v1.8.8: **re-exporta** el parser original de `lang::bmo::parser`.
//! En la próxima fase, el parser se reescribirá para emitir directamente
//! `common::ast::Module` (BMO IR) en vez de `lang::bmo::parser::ast::Ast`.

#![allow(dead_code)]

pub use crate::lang::bmo::parser::ast as ast;
pub use crate::lang::bmo::parser::ast::Parser;
pub use ast::{Ast, Stmt, Expr, Param, TypeAnnotation, BinOp, UnaryOp, ExternItem};
