//! C Abstract Syntax Tree — top-level program nodes.

use super::expr::Expr;
use super::types::TypeSpec;
use super::stmt::Stmt;

#[derive(Debug, Clone, PartialEq)]
pub struct StructMember {
    pub typ: TypeSpec,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GlobalDecl {
    Var(TypeSpec, String, Option<Expr>),
    Struct(String, Vec<StructMember>),
    Union(String, Vec<StructMember>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub ret_type: TypeSpec,
    pub name: String,
    pub params: Vec<super::types::Param>,
    pub var_count: u32,
    pub var_names: Vec<String>,
    pub body: Vec<Stmt>,
    pub line: usize,
    /// ¿Declara `...`? Lo necesita el codegen para saber si `__va_arg()` tiene
    /// algo que leer — y para poder DECIRLO cuando no lo tiene.
    pub variadica: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub globals: Vec<GlobalDecl>,
    pub functions: Vec<Function>,
    pub exported: Vec<String>,
}

impl Program {
    pub fn new() -> Self {
        Self { globals: Vec::new(), functions: Vec::new(), exported: Vec::new() }
    }
}
