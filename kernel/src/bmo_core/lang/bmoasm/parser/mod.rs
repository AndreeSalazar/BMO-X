//! Parser — convierte tokens en AST.

pub mod ast;
pub mod parse;
pub mod error;

pub use ast::{Ast, Stmt, Expr, BinOp, Type};
pub use parse::Parser;
#[allow(unused_imports)]
pub use error::{ParseError, ParseResult};
