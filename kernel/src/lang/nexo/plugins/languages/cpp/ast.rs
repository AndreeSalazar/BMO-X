//! C++ AST — extends the C AST with classes, methods, virtual dispatch.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

pub use super::super::c::ast::{CType, CExpr, CStmt, CItem, CAst, CParam, CBinOp, CUnaryOp};

/// C++ access specifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CppAccess {
    Public,
    Private,
    Protected,
}

/// A field or method inside a C++ class.
#[derive(Debug, Clone)]
pub struct ClassMember {
    pub access: CppAccess,
    pub kind: ClassMemberKind,
}

#[derive(Debug, Clone)]
pub enum ClassMemberKind {
    Field { ty: CType, name: String, init: Option<CExpr> },
    Method {
        ret: CType,
        name: String,
        params: Vec<CParam>,
        body: Vec<CStmt>,
        is_virtual: bool,
        is_static: bool,
        is_const: bool,
    },
    Constructor {
        params: Vec<CParam>,
        member_init: Vec<(String, CExpr)>,
        body: Vec<CStmt>,
    },
    Destructor {
        body: Vec<CStmt>,
    },
}

/// A complete C++ class definition.
#[derive(Debug, Clone)]
pub struct CppClass {
    pub name: String,
    pub parent: Option<String>,
    pub members: Vec<ClassMember>,
}

/// C++ top-level item (extends CItem with classes).
#[derive(Debug, Clone)]
pub enum CppItem {
    CItem(CItem),
    Class(CppClass),
}

/// C++ program AST.
#[derive(Debug, Clone, Default)]
pub struct CppAst {
    pub items: Vec<CppItem>,
}
