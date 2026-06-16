//! C AST — Abstract Syntax Tree types for C source code.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// C type representation.
#[derive(Debug, Clone)]
pub enum CType {
    Int,
    UnsignedInt,
    Long,
    UnsignedLong,
    Char,
    Void,
    Short,
    Float,
    Double,
    Ptr(Box<CType>),
    Array(Box<CType>, u64),
    Struct(String),
    Named(String),
}

/// C expression.
#[derive(Debug, Clone)]
pub enum CExpr {
    IntLit(u64),
    StrLit(String),
    CharLit(u8),
    Ident(String),
    Binary(CBinOp, Box<CExpr>, Box<CExpr>),
    Unary(CUnaryOp, Box<CExpr>),
    Call(String, Vec<CExpr>),
    Assign(Box<CExpr>, Box<CExpr>),
    Member(Box<CExpr>, String),
    ArrowMember(Box<CExpr>, String),
    Sizeof(CType),
    ArrayIndex(Box<CExpr>, Box<CExpr>),
    Cast(CType, Box<CExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CBinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor,
    Shl, Shr,
    Assign, AddAssign, SubAssign, MulAssign, DivAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CUnaryOp {
    Neg, Not, BitNot, PreInc, PreDec, PostInc, PostDec, Deref, AddrOf,
}

/// C statement.
#[derive(Debug, Clone)]
pub enum CStmt {
    Empty,
    Expr(CExpr),
    Decl { ty: CType, name: String, init: Option<CExpr> },
    If { cond: CExpr, then_body: Box<CStmt>, else_body: Option<Box<CStmt>> },
    While { cond: CExpr, body: Box<CStmt> },
    For { init: Option<Box<CStmt>>, cond: Option<CExpr>, update: Option<CExpr>, body: Box<CStmt> },
    Do { cond: CExpr, body: Box<CStmt> },
    Block(Vec<CStmt>),
    Return(Option<CExpr>),
    Break,
    Continue,
}

/// C function parameter.
#[derive(Debug, Clone)]
pub struct CParam {
    pub ty: CType,
    pub name: String,
}

/// C top-level declaration.
#[derive(Debug, Clone)]
pub enum CItem {
    Function {
        name: String,
        ret: CType,
        params: Vec<CParam>,
        body: Option<Vec<CStmt>>,
        is_static: bool,
        is_extern: bool,
    },
    Struct {
        name: String,
        fields: Vec<(CType, String)>,
    },
    Typedef {
        name: String,
        ty: CType,
    },
    GlobalVar {
        ty: CType,
        name: String,
        init: Option<CExpr>,
        is_static: bool,
        is_extern: bool,
    },
}

/// C program AST.
#[derive(Debug, Clone, Default)]
pub struct CAst {
    pub items: Vec<CItem>,
}
