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
    Union(String),
    Enum(String),
    Named(String),
}

impl CType {
    /// Get size in bytes (for x86_64)
    pub fn size_bytes(&self) -> u64 {
        match self {
            CType::Char => 1,
            CType::Short => 2,
            CType::Int | CType::UnsignedInt => 4,
            CType::Long | CType::UnsignedLong => 8,
            CType::Float => 4,
            CType::Double => 8,
            CType::Void => 0,
            CType::Ptr(_) => 8,        // 64-bit pointer
            CType::Array(inner, len) => inner.size_bytes() * len,
            CType::Struct(_) => 0,     // Would need struct table
            CType::Union(_) => 0,      // Would need union table
            CType::Enum(_) => 4,       // Enums are int-sized
            CType::Named(_) => 8,      // Assume 8 bytes for unknown
        }
    }
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
    SizeofExpr(Box<CExpr>),    // sizeof(expr)
    ArrayIndex(Box<CExpr>, Box<CExpr>),
    Cast(CType, Box<CExpr>),
    Ternary(Box<CExpr>, Box<CExpr>, Box<CExpr>),  // ? :
    Comma(Vec<CExpr>),         // a, b, c
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CBinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor,
    Shl, Shr,
    Assign, AddAssign, SubAssign, MulAssign, DivAssign,
    BitAndAssign, BitOrAssign, BitXorAssign,
    ShlAssign, ShrAssign,
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
    Switch { expr: CExpr, cases: Vec<CCase>, default: Option<Vec<CStmt>> },
    Block(Vec<CStmt>),
    Return(Option<CExpr>),
    Break,
    Continue,
    Label(String),
    Goto(String),
}

/// C case in switch statement
#[derive(Debug, Clone)]
pub struct CCase {
    pub value: CExpr,
    pub stmts: Vec<CStmt>,
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
    Union {
        name: String,
        fields: Vec<(CType, String)>,
    },
    Enum {
        name: String,
        variants: Vec<(String, Option<i64>)>,
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
    Macro {
        name: String,
        params: Option<Vec<String>>,
        body: String,
    },
    Include(String),
}

/// C program AST.
#[derive(Debug, Clone, Default)]
pub struct CAst {
    pub items: Vec<CItem>,
}
