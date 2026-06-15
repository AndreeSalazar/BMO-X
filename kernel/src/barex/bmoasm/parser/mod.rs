//! Parser — convierte tokens en AST.

pub mod ast;
pub mod parse;

pub use ast::{Ast, Stmt, Expr};
pub use parse::Parser;
