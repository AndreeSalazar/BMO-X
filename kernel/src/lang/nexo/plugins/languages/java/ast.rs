//! Java AST — essential subset.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Java primitive type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JPrim {
    Void, Boolean, Byte, Short, Int, Long, Float, Double, Char,
}

impl JPrim {
    pub fn name(self) -> &'static str {
        match self {
            JPrim::Void => "void",
            JPrim::Boolean => "boolean",
            JPrim::Byte => "byte",
            JPrim::Short => "short",
            JPrim::Int => "int",
            JPrim::Long => "long",
            JPrim::Float => "float",
            JPrim::Double => "double",
            JPrim::Char => "char",
        }
    }
}

/// Java type.
#[derive(Debug, Clone)]
pub enum JType {
    Prim(JPrim),
    Class(String),
    Array(Box<JType>),
}

/// Java modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JMod {
    Public, Private, Protected, Static, Final, Abstract,
}

/// Java member (field or method).
#[derive(Debug, Clone)]
pub struct JMember {
    pub mods: Vec<JMod>,
    pub kind: JMemberKind,
}

#[derive(Debug, Clone)]
pub enum JMemberKind {
    Field { ty: JType, name: String, init: Option<JExpr> },
    Method {
        ret: JType,
        name: String,
        params: Vec<JParam>,
        body: Vec<JStmt>,
        is_abstract: bool,
    },
    Constructor {
        params: Vec<JParam>,
        body: Vec<JStmt>,
    },
}

#[derive(Debug, Clone)]
pub struct JParam {
    pub ty: JType,
    pub name: String,
}

/// Java class or interface.
#[derive(Debug, Clone)]
pub struct JClass {
    pub mods: Vec<JMod>,
    pub name: String,
    pub parent: Option<String>,
    pub implements: Vec<String>,
    pub is_interface: bool,
    pub members: Vec<JMember>,
}

/// Java binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JBinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

/// Java expression.
#[derive(Debug, Clone)]
pub enum JExpr {
    IntLit(i64),
    FloatLit(u64),
    StrLit(String),
    BoolLit(bool),
    Null,
    This,
    Name(String),
    Bin(JBinOp, Box<JExpr>, Box<JExpr>),
    Unary(JUnOp, Box<JExpr>),
    Call(Box<JExpr>, Vec<JExpr>),
    Field(Box<JExpr>, String),
    Index(Box<JExpr>, Box<JExpr>),
    New(String, Vec<JExpr>),
    NewArray(JType, Box<JExpr>),
    Cast(JType, Box<JExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JUnOp {
    Neg, Not, BitNot,
}

/// Java statement.
#[derive(Debug, Clone)]
pub enum JStmt {
    Expr(JExpr),
    LocalDecl { ty: JType, name: String, init: Option<JExpr> },
    Assign(JExpr, JExpr),
    If { cond: JExpr, then_body: Vec<JStmt>, else_body: Option<Vec<JStmt>> },
    While { cond: JExpr, body: Vec<JStmt> },
    For { init: Option<Box<JStmt>>, cond: Option<JExpr>, update: Option<JExpr>, body: Vec<JStmt> },
    Return(Option<JExpr>),
    Block(Vec<JStmt>),
    Break,
    Continue,
    Throw(JExpr),
    Try { body: Vec<JStmt>, catches: Vec<JCatch>, finally: Option<Vec<JStmt>> },
}

#[derive(Debug, Clone)]
pub struct JCatch {
    pub catch_type: Option<String>,
    pub name: String,
    pub body: Vec<JStmt>,
}

/// Java top-level item.
#[derive(Debug, Clone)]
pub enum JItem {
    Class(JClass),
}

/// Java program AST.
#[derive(Debug, Clone, Default)]
pub struct JAst {
    pub items: Vec<JItem>,
}
