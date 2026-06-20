//! Python AST — subset for BMO.
//!
//! Models a small Python program: literals, names, calls, binary ops,
//! control flow, function defs, class defs, imports.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Python literal.
#[derive(Debug, Clone)]
pub enum PyLiteral {
    Int(i64),
    Float(u64),  // bits
    Str(String),
    Bool(bool),
    None,
    List(Vec<PyExpr>),
    Dict(Vec<(PyExpr, PyExpr)>),
    Tuple(Vec<PyExpr>),
}

/// Python binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyBinOp {
    Add, Sub, Mul, Div, Mod, FloorDiv,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
    Pow,
}

/// Python unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyUnaryOp {
    Neg, Not, BitNot,
}

/// Python expression.
#[derive(Debug, Clone)]
pub enum PyExpr {
    Literal(PyLiteral),
    Name(String),
    Bin(PyBinOp, Box<PyExpr>, Box<PyExpr>),
    Unary(PyUnaryOp, Box<PyExpr>),
    Call(Box<PyExpr>, Vec<PyExpr>),  // callee + args (callee may be Name or Attribute)
    MethodCall(Box<PyExpr>, String, Vec<PyExpr>),
    Attribute(Box<PyExpr>, String),
    Index(Box<PyExpr>, Box<PyExpr>),
    Slice(Box<PyExpr>, Option<Box<PyExpr>>, Option<Box<PyExpr>>),
    Lambda(Vec<String>, Box<PyStmt>),  // args, body
    List(Vec<PyExpr>),
    Dict(Vec<(PyExpr, PyExpr)>),
    Tuple(Vec<PyExpr>),
}

/// Python import statement kind.
#[derive(Debug, Clone)]
pub enum PyImport {
    /// `import foo`
    Module(String),
    /// `from foo import bar`
    From(String, Vec<String>),
    /// `import foo as bar`
    As(String, String),
}

/// Python statement.
#[derive(Debug, Clone)]
pub enum PyStmt {
    Expr(PyExpr),
    Assign(Vec<PyExpr>, PyExpr),  // target(s), value (a, b = c)
    AugAssign(PyExpr, PyBinOp, PyExpr),
    If { cond: PyExpr, then_body: Vec<PyStmt>, elif_branches: Vec<(PyExpr, Vec<PyStmt>)>, else_body: Option<Vec<PyStmt>> },
    While { cond: PyExpr, body: Vec<PyStmt> },
    For { var: String, iter: PyExpr, body: Vec<PyStmt> },
    Return(Option<PyExpr>),
    Break,
    Continue,
    Pass,
    FuncDef { name: String, params: Vec<String>, body: Vec<PyStmt> },
    ClassDef { name: String, parent: Option<String>, body: Vec<PyStmt> },
    Import(PyImport),
    Try { body: Vec<PyStmt>, except_name: Option<String>, except_body: Vec<PyStmt>, finally_body: Option<Vec<PyStmt>> },
    With { ctx: PyExpr, body: Vec<PyStmt> },
    Block(Vec<PyStmt>),
}

/// Python program AST.
#[derive(Debug, Clone, Default)]
pub struct PyAst {
    pub items: Vec<PyStmt>,
}
