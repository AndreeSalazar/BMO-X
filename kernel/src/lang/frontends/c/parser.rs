//! C Parser — convierte tokens a C AST.
//!
//! v1.8.8: re-exporta el parser C existente. En la próxima fase se
//! reescribirá para emitir errores con `DiagCode` canónico.

#![allow(dead_code)]

pub use crate::lang::bmo::plugins::languages::c::parser::Parser;
pub use crate::lang::bmo::plugins::languages::c::ast as ast_compat;
pub use crate::lang::bmo::plugins::languages::c::ast::{CAst, CItem, CExpr, CStmt, CType, CBinOp, CUnaryOp};
