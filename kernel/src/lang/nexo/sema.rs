//! ÑEXO Sema — Análisis semántico.
//!
//! Valida tipos, scopes, y genera información para codegen.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::String;

use crate::barex::{BxError, BxResult};
use super::parser::{Ast, Stmt, Expr, TypeAnnotation};

/// Variable info for scope tracking.
#[derive(Debug, Clone)]
pub struct VarInfo {
    pub name: String,
    pub ty: TypeAnnotation,
    pub offset: i32, // stack offset from RBP
}

/// Scope for variable tracking.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub vars: Vec<VarInfo>,
    pub frame_size: u32,
}

impl Scope {
    pub fn lookup(&self, name: &str) -> Option<&VarInfo> {
        self.vars.iter().rev().find(|v| v.name == name)
    }

    pub fn push(&mut self, name: String, ty: TypeAnnotation) {
        let offset = -(self.frame_size as i32) - 8;
        self.frame_size += 8;
        self.vars.push(VarInfo { name, ty, offset });
    }
}

/// Semantic analyzer for ÑEXO.
pub struct Sema;

impl Sema {
    pub fn new() -> Self { Self }

    pub fn check(&self, ast: &Ast) -> BxResult<()> {
        let mut scope = Scope::default();
        for item in &ast.items {
            self.check_stmt(item, &mut scope)?;
        }
        Ok(())
    }

    fn check_stmt(&self, stmt: &Stmt, scope: &mut Scope) -> BxResult<()> {
        match stmt {
            Stmt::Let { name, ty, value } => {
                if let Some(val) = value {
                    self.check_expr(val, scope)?;
                }
                let annotated_ty = ty.clone().unwrap_or(TypeAnnotation::Named(String::from("num")));
                scope.push(name.clone(), annotated_ty);
            }
            Stmt::FnDecl { params, body, .. } => {
                for p in params {
                    scope.push(p.name.clone(), p.ty.clone());
                }
                for s in body {
                    self.check_stmt(s, scope)?;
                }
            }
            Stmt::If { cond, then_body, else_body } => {
                self.check_expr(cond, scope)?;
                for s in then_body { self.check_stmt(s, scope)?; }
                if let Some(eb) = else_body {
                    for s in eb { self.check_stmt(s, scope)?; }
                }
            }
            Stmt::While { cond, body } => {
                self.check_expr(cond, scope)?;
                for s in body { self.check_stmt(s, scope)?; }
            }
            Stmt::Return(Some(expr)) => { self.check_expr(expr, scope)?; }
            Stmt::Return(None) => {}
            Stmt::Block(stmts) => {
                for s in stmts { self.check_stmt(s, scope)?; }
            }
            Stmt::ExprStmt(expr) => { self.check_expr(expr, scope)?; }
            _ => {}
        }
        Ok(())
    }

    fn check_expr(&self, expr: &Expr, scope: &Scope) -> BxResult<()> {
        match expr {
            Expr::Ident(name) => {
                if scope.lookup(name).is_none() {
                    return Err(BxError::InvalidArgument);
                }
            }
            Expr::Binary(_, left, right) => {
                self.check_expr(left, scope)?;
                self.check_expr(right, scope)?;
            }
            Expr::Unary(_, inner) => { self.check_expr(inner, scope)?; }
            Expr::Call(func, args) => {
                self.check_expr(func, scope)?;
                for a in args { self.check_expr(a, scope)?; }
            }
            _ => {}
        }
        Ok(())
    }
}
