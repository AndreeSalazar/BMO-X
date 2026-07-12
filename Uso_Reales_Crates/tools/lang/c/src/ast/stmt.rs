//! C Abstract Syntax Tree — statement nodes.

use super::expr::Expr;
use super::types::TypeSpec;

#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub value: Option<i64>,
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Printf(String),
    PrintfLn(String),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    For(Option<Expr>, Option<Expr>, Option<Expr>, Box<Stmt>),
    Switch(Expr, Vec<Case>),
    Break,
    Continue,
    Return(Option<Expr>),
    DeclAssign(TypeSpec, String, Option<Expr>),
    Expr(Expr),
    Block(Vec<Stmt>),
    Goto(String),
    Label(String),
}
