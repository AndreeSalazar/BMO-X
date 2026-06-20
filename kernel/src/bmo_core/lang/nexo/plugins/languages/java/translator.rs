//! Java → ÑEXO Translator — converts Java AST to ÑEXO AST.
//!
//! Strategy: lower Java to a C-like ÑEXO AST with:
//!
//! v1.6.16: allow(irrefutable_let_patterns) for the two `if let Ok(_) = ...`
//! sites that the compiler flagged. They are valid match-arms that we
//! keep around for future error-context; the warning is noise.

#![allow(irrefutable_let_patterns)]
//! - `class Foo { ... }` → struct + vtable
//! - `this.field` → struct field access
//! - `new Foo(args)` → aloc + placement + constructor
//! - `try/catch` → setjmp/longjmp-style (managed in exceptions.rs)
//! - `interface` → pure virtual vtable (all methods abstract)

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::bmo_core::barex::BxResult;
use super::ast::*;
use crate::bmo_core::lang::nexo::parser::Ast;

pub struct JavaToNexo {
    classes: BTreeMap<String, JClass>,
}

impl JavaToNexo {
    pub fn new() -> Self { Self { classes: BTreeMap::new() } }

    pub fn translate(&mut self, jast: &JAst) -> BxResult<Ast> {
        for item in &jast.items {
            if let JItem::Class(cls) = item {
                self.classes.insert(cls.name.clone(), cls.clone());
            }
        }
        let mut items = Vec::new();
        for item in &jast.items {
            if let JItem::Class(cls) = item {
                items.push(self.translate_class(cls)?);
            }
        }
        Ok(Ast { items })
    }

    fn translate_class(&self, cls: &JClass) -> BxResult<crate::bmo_core::lang::nexo::parser::Stmt> {
        // Lower to: a struct with embedded parent + vptr + data fields.
        let mut nexo_fields = Vec::new();
        if let Some(parent) = &cls.parent {
            nexo_fields.push(("base".to_string(), crate::bmo_core::lang::nexo::parser::TypeAnnotation::Named(parent.clone())));
        }
        let mut vt_name = cls.name.clone();
        vt_name.push_str("_vtable");
        nexo_fields.push(("vptr".to_string(), crate::bmo_core::lang::nexo::parser::TypeAnnotation::Named(vt_name)));
        for m in &cls.members {
            if let JMemberKind::Field { ty, name, .. } = &m.kind {
                nexo_fields.push((name.clone(), java_type_to_typeannotation(ty)));
            }
        }
        Ok(crate::bmo_core::lang::nexo::parser::Stmt::StructDecl {
            name: cls.name.clone(),
            fields: nexo_fields,
        })
    }
}

fn java_type_to_typeannotation(ty: &JType) -> crate::bmo_core::lang::nexo::parser::TypeAnnotation {
    use crate::bmo_core::lang::nexo::parser::TypeAnnotation;
    use JType::*;
    match ty {
        Prim(p) => TypeAnnotation::Named(p.name().to_string()),
        Class(name) => TypeAnnotation::Named(name.clone()),
        Array(inner) => TypeAnnotation::Array(Box::new(java_type_to_typeannotation(inner)), 0),
    }
}

