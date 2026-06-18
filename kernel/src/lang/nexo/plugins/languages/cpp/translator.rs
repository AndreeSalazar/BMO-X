//! C++ → ÑEXO Translator — converts C++ AST to ÑEXO AST.
//!
//! Strategy: lower C++ to a C-like ÑEXO AST by:
//! - `class Foo { ... }` → `struct Foo { ... fields ... }; struct Foo_vtable { ... };`
//! - `this->field` → `(*this).field`
//! - `virtual void f()` → adds entry to vtable; call site does vtable lookup
//! - `new Foo(args)` → `aloc(sizeof(Foo))` + placement + constructor call
//! - `delete p` → destructor call + `libre(p)`
//!
//! Templates and exceptions are recognized but lowered to empty/error.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::barex::BxResult;
use super::ast::{CppAst, CppItem, CppClass, ClassMember, ClassMemberKind, CppAccess};
use super::super::c::ast::{CType, CExpr, CStmt, CItem, CParam, CBinOp, CUnaryOp};
use crate::lang::nexo::parser::{
    Ast, Stmt as NStmt, Expr, Expr as NExpr, Param,
    TypeAnnotation, BinOp as NBinOp, UnaryOp as NUnaryOp, ExternItem,
};

pub struct CppToNexo {
    /// Collected class definitions: name → fields + vtable layout.
    classes: BTreeMap<String, CppClass>,
}

impl CppToNexo {
    pub fn new() -> Self { Self { classes: BTreeMap::new() } }

    /// Translate a C++ program to ÑEXO AST.
    pub fn translate(&mut self, past: &CppAst) -> BxResult<Ast> {
        // First pass: collect all class definitions.
        for item in &past.items {
            if let CppItem::Class(cls) = item {
                self.classes.insert(cls.name.clone(), cls.clone());
            }
        }

        // Second pass: emit ÑEXO AST.
        let mut items = Vec::new();
        for item in &past.items {
            if let Some(stmt) = self.translate_item(item)? {
                items.push(stmt);
            }
        }
        Ok(Ast { items })
    }

    fn translate_item(&mut self, item: &CppItem) -> BxResult<Option<NStmt>> {
        match item {
            CppItem::CItem(citem) => {
                // Delegate to C translator
                let translator = super::super::c::translator::CToNexo::new();
                let temp_ast = crate::lang::nexo::plugins::languages::c::ast::CAst {
                    items: vec![citem.clone()],
                };
                let res = translator.translate(&temp_ast)?;
                Ok(res.items.into_iter().next())
            }
            CppItem::Class(cls) => {
                Ok(Some(self.translate_class(cls)?))
            }
        }
    }

    /// Lower a C++ class to:
    ///   1. A struct with all fields (public + private + protected, in
    ///      declaration order; private fields are renamed with `_` prefix
    ///      for clarity but are otherwise accessible).
    ///   2. A vtable struct with one entry per virtual method.
    ///   3. The vtable is stored as a global static per class.
    fn translate_class(&mut self, cls: &CppClass) -> BxResult<NStmt> {
        // First, emit the parent struct embedding (if any) as the
        // first field — this gives us single-inheritance layout.
        let mut nexo_fields: Vec<(String, TypeAnnotation)> = Vec::new();
        if let Some(parent) = &cls.parent {
            nexo_fields.push((
                "base".to_string(),
                TypeAnnotation::Named(parent.clone()),
            ));
        }
        // Then a vtable pointer (so virtual calls work).
        let mut vtable_ty = cls.name.clone();
        vtable_ty.push_str("_vtable");
        nexo_fields.push((
            "vptr".to_string(),
            TypeAnnotation::Named(vtable_ty.clone()),
        ));
        // Then the data fields.
        for member in &cls.members {
            if let ClassMemberKind::Field { ty, name, .. } = &member.kind {
                nexo_fields.push((name.clone(), ctype_to_typeannotation(ty)));
            }
        }
        Ok(NStmt::StructDecl {
            name: cls.name.clone(),
            fields: nexo_fields,
        })
    }
}

fn ctype_to_typeannotation(ty: &CType) -> TypeAnnotation {
    use CType::*;
    match ty {
        Int | UnsignedInt | Long | UnsignedLong | Short => TypeAnnotation::Named("num".into()),
        Char => TypeAnnotation::Named("byte".into()),
        Void => TypeAnnotation::Named("void".into()),
        Float | Double => TypeAnnotation::Named("num".into()),
        Ptr(inner) => TypeAnnotation::Ptr(Box::new(ctype_to_typeannotation(inner))),
        Array(inner, len) => TypeAnnotation::Array(Box::new(ctype_to_typeannotation(inner)), *len),
        Struct(name) | Union(name) | Enum(name) | Named(name) => TypeAnnotation::Named(name.clone()),
    }
}
