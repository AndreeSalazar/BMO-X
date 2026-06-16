//! C → ÑEXO Translator — converts C AST to ÑEXO AST.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::barex::BxResult;
use super::ast::{CType, CExpr, CBinOp, CUnaryOp, CStmt, CItem, CAst};
use crate::lang::nexo::parser::{Ast, Stmt as NStmt, Expr, Param, TypeAnnotation, BinOp, UnaryOp, ExternItem};

/// Translates C AST to ÑEXO AST.
pub struct CToNexo;

impl CToNexo {
    pub fn new() -> Self { Self }

    /// Translate a C program to ÑEXO AST.
    pub fn translate(&self, cast: &CAst) -> BxResult<Ast> {
        let mut items = Vec::new();
        for item in &cast.items {
            if let Some(stmt) = self.translate_item(item)? {
                items.push(stmt);
            }
        }
        Ok(Ast { items })
    }

    fn translate_item(&self, item: &CItem) -> BxResult<Option<NStmt>> {
        match item {
            CItem::Function { name, ret, params, body, is_static: _, is_extern } => {
                if *is_extern {
                    let nexo_params: Vec<Param> = params.iter().map(|p| Param {
                        name: p.name.clone(),
                        ty: self.translate_type(&p.ty),
                    }).collect();
                    return Ok(Some(NStmt::Extern { items: vec![ExternItem::Fn {
                        name: name.clone(),
                        params: nexo_params,
                        ret: Some(self.translate_type(ret)),
                    }] }));
                }

                let nexo_params: Vec<Param> = params.iter().map(|p| Param {
                    name: p.name.clone(),
                    ty: self.translate_type(&p.ty),
                }).collect();

                let nexo_ret = Some(self.translate_type(ret));
                let nexo_body = if let Some(stmts) = body {
                    stmts.iter().filter_map(|s| self.translate_stmt(s).ok().flatten()).collect()
                } else {
                    Vec::new()
                };

                Ok(Some(NStmt::FnDecl {
                    name: name.clone(),
                    params: nexo_params,
                    ret: nexo_ret,
                    body: nexo_body,
                }))
            }
            CItem::Struct { name, fields } => {
                let nexo_fields: Vec<(String, TypeAnnotation)> = fields.iter()
                    .map(|(ty, fname)| (fname.clone(), self.translate_type(ty)))
                    .collect();
                Ok(Some(NStmt::StructDecl { name: name.clone(), fields: nexo_fields }))
            }
            CItem::Typedef { .. } => Ok(None),
            CItem::GlobalVar { ty: _, name, init, is_static: _, is_extern } => {
                if *is_extern {
                    return Ok(None);
                }
                let value = init.as_ref().map(|e| self.translate_expr(e)).transpose()?;
                Ok(Some(NStmt::Let {
                    name: name.clone(),
                    ty: None,
                    value,
                }))
            }
        }
    }

    fn translate_stmt(&self, stmt: &CStmt) -> BxResult<Option<NStmt>> {
        match stmt {
            CStmt::Empty => Ok(None),
            CStmt::Expr(expr) => {
                let nexo_expr = self.translate_expr(expr)?;
                Ok(Some(NStmt::ExprStmt(nexo_expr)))
            }
            CStmt::Decl { ty: _, name, init } => {
                let value = init.as_ref().map(|e| self.translate_expr(e)).transpose()?;
                Ok(Some(NStmt::Let {
                    name: name.clone(),
                    ty: None,
                    value,
                }))
            }
            CStmt::If { cond, then_body, else_body } => {
                let nexo_cond = self.translate_expr(cond)?;
                let then = self.translate_stmt(then_body)?.into_iter().collect();
                let else_b = else_body.as_ref()
                    .map(|eb| self.translate_stmt(eb).map(|s| s.into_iter().collect()))
                    .transpose()?;
                Ok(Some(NStmt::If { cond: nexo_cond, then_body: then, else_body: else_b }))
            }
            CStmt::While { cond, body } => {
                let nexo_cond = self.translate_expr(cond)?;
                let nexo_body = self.translate_stmt(body)?.into_iter().collect();
                Ok(Some(NStmt::While { cond: nexo_cond, body: nexo_body }))
            }
            CStmt::For { init, cond, update: _, body } => {
                let mut stmts = Vec::new();
                if let Some(init_stmt) = init {
                    if let Some(s) = self.translate_stmt(init_stmt)? {
                        stmts.push(s);
                    }
                }
                let cond_expr = cond.as_ref()
                    .map(|e| self.translate_expr(e))
                    .transpose()?
                    .unwrap_or(Expr::LitBool(true));
                let nexo_body = self.translate_stmt(body)?.into_iter().collect();
                let while_stmt = NStmt::While { cond: cond_expr, body: nexo_body };
                stmts.push(while_stmt);
                Ok(Some(NStmt::Block(stmts)))
            }
            CStmt::Do { cond, body } => {
                let nexo_cond = self.translate_expr(cond)?;
                let nexo_body: Vec<NStmt> = self.translate_stmt(body)?.into_iter().collect();
                let mut stmts = nexo_body;
                stmts.push(NStmt::While { cond: nexo_cond, body: Vec::new() });
                Ok(Some(NStmt::Block(stmts)))
            }
            CStmt::Block(stmts) => {
                let nexo_stmts: Vec<NStmt> = stmts.iter()
                    .filter_map(|s| self.translate_stmt(s).ok().flatten())
                    .collect();
                Ok(Some(NStmt::Block(nexo_stmts)))
            }
            CStmt::Return(expr) => {
                let val = expr.as_ref().map(|e| self.translate_expr(e)).transpose()?;
                Ok(Some(NStmt::Return(val)))
            }
            CStmt::Break => Ok(Some(NStmt::Break)),
            CStmt::Continue => Ok(Some(NStmt::Continue)),
        }
    }

    fn translate_expr(&self, expr: &CExpr) -> BxResult<Expr> {
        match expr {
            CExpr::IntLit(n) => Ok(Expr::LitInt(*n)),
            CExpr::StrLit(s) => Ok(Expr::LitStr(s.clone())),
            CExpr::CharLit(c) => Ok(Expr::LitByte(*c)),
            CExpr::Ident(name) => Ok(Expr::Ident(name.clone())),
            CExpr::Binary(op, left, right) => {
                let l = self.translate_expr(left)?;
                let r = self.translate_expr(right)?;
                let nexo_op = self.translate_binop(*op);
                Ok(Expr::Binary(nexo_op, Box::new(l), Box::new(r)))
            }
            CExpr::Unary(op, inner) => {
                let e = self.translate_expr(inner)?;
                match op {
                    CUnaryOp::Neg => Ok(Expr::Unary(UnaryOp::Neg, Box::new(e))),
                    CUnaryOp::Not => Ok(Expr::Unary(UnaryOp::Not, Box::new(e))),
                    CUnaryOp::Deref => Ok(Expr::Unary(UnaryOp::Deref, Box::new(e))),
                    CUnaryOp::AddrOf => Ok(Expr::Unary(UnaryOp::Ref, Box::new(e))),
                    CUnaryOp::PreInc => {
                        Ok(Expr::Binary(BinOp::Add, Box::new(e), Box::new(Expr::LitInt(1))))
                    }
                    CUnaryOp::PreDec => {
                        Ok(Expr::Binary(BinOp::Sub, Box::new(e), Box::new(Expr::LitInt(1))))
                    }
                    CUnaryOp::PostInc => {
                        Ok(Expr::Binary(BinOp::Add, Box::new(e), Box::new(Expr::LitInt(1))))
                    }
                    CUnaryOp::PostDec => {
                        Ok(Expr::Binary(BinOp::Sub, Box::new(e), Box::new(Expr::LitInt(1))))
                    }
                    CUnaryOp::BitNot => Ok(Expr::Unary(UnaryOp::Not, Box::new(e))),
                }
            }
            CExpr::Call(name, args) => {
                let nexo_args: Vec<Expr> = args.iter()
                    .map(|a| self.translate_expr(a))
                    .collect::<BxResult<Vec<_>>>()?;
                Ok(Expr::Call(name.clone(), nexo_args))
            }
            CExpr::Assign(left, right) => {
                if let CExpr::Ident(name) = left.as_ref() {
                    let val = self.translate_expr(right)?;
                    Ok(Expr::Binary(BinOp::Add, Box::new(Expr::Ident(name.clone())), Box::new(val)))
                } else {
                    let l = self.translate_expr(left)?;
                    let r = self.translate_expr(right)?;
                    Ok(Expr::Binary(BinOp::Add, Box::new(l), Box::new(r)))
                }
            }
            CExpr::Member(obj, field) | CExpr::ArrowMember(obj, field) => {
                let o = self.translate_expr(obj)?;
                Ok(Expr::Field(Box::new(o), field.clone()))
            }
            CExpr::Sizeof(_) => Ok(Expr::LitInt(8)),
            CExpr::ArrayIndex(obj, idx) => {
                let o = self.translate_expr(obj)?;
                let i = self.translate_expr(idx)?;
                Ok(Expr::Index(Box::new(o), Box::new(i)))
            }
            CExpr::Cast(_, inner) => self.translate_expr(inner),
        }
    }

    fn translate_type(&self, ty: &CType) -> TypeAnnotation {
        match ty {
            CType::Int | CType::Long | CType::Short => TypeAnnotation::Named(String::from("num")),
            CType::UnsignedInt | CType::UnsignedLong => TypeAnnotation::Named(String::from("num")),
            CType::Char => TypeAnnotation::Named(String::from("byte")),
            CType::Void => TypeAnnotation::Named(String::from("void")),
            CType::Float | CType::Double => TypeAnnotation::Named(String::from("num")),
            CType::Ptr(inner) => TypeAnnotation::Ptr(Box::new(self.translate_type(inner))),
            CType::Array(inner, _) => TypeAnnotation::Array(Box::new(self.translate_type(inner)), 0),
            CType::Struct(name) => TypeAnnotation::Named(name.clone()),
            CType::Named(name) => TypeAnnotation::Named(name.clone()),
        }
    }

    fn translate_binop(&self, op: CBinOp) -> BinOp {
        match op {
            CBinOp::Add | CBinOp::AddAssign => BinOp::Add,
            CBinOp::Sub | CBinOp::SubAssign => BinOp::Sub,
            CBinOp::Mul | CBinOp::MulAssign => BinOp::Mul,
            CBinOp::Div | CBinOp::DivAssign => BinOp::Div,
            CBinOp::Mod => BinOp::Mod,
            CBinOp::Eq => BinOp::Eq,
            CBinOp::Ne => BinOp::Ne,
            CBinOp::Lt => BinOp::Lt,
            CBinOp::Gt => BinOp::Gt,
            CBinOp::Le => BinOp::Le,
            CBinOp::Ge => BinOp::Ge,
            CBinOp::And => BinOp::Land,
            CBinOp::Or => BinOp::Lor,
            CBinOp::BitAnd => BinOp::And,
            CBinOp::BitOr => BinOp::Or,
            CBinOp::BitXor => BinOp::Xor,
            CBinOp::Shl => BinOp::Shl,
            CBinOp::Shr => BinOp::Shr,
            CBinOp::Assign => BinOp::Add, // Placeholder
        }
    }
}
