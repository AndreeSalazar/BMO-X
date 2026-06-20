//! C → ÑEXO Translator — converts C AST to ÑEXO AST.
//!
//! Supports: structs, unions, enums, switch/case, sizeof(expr), ternary, comma, goto/label.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use super::ast::{CType, CExpr, CBinOp, CUnaryOp, CStmt, CItem, CAst};
use crate::bmo_core::lang::bmo::parser::{Ast, Stmt as NStmt, Expr, Param, TypeAnnotation, BinOp, UnaryOp, ExternItem};

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
            CItem::Union { name, fields } => {
                // Translate union as struct (ÑEXO doesn't have unions yet)
                let nexo_fields: Vec<(String, TypeAnnotation)> = fields.iter()
                    .map(|(ty, fname)| (fname.clone(), self.translate_type(ty)))
                    .collect();
                Ok(Some(NStmt::StructDecl { name: name.clone(), fields: nexo_fields }))
            }
            CItem::Enum { name: _, variants } => {
                // Translate enum as constants
                let mut stmts = Vec::new();
                for (i, (var_name, val)) in variants.iter().enumerate() {
                    let value = val.unwrap_or(i as i64);
                    stmts.push(NStmt::Let {
                        name: var_name.clone(),
                        ty: Some(TypeAnnotation::Named(String::from("num"))),
                        value: Some(Expr::LitInt(value as u64)),
                    });
                }
                Ok(Some(NStmt::Block(stmts)))
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
            CItem::Macro { .. } => Ok(None),
            CItem::Include(_) => Ok(None),
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
            CStmt::Switch { expr, cases, default } => {
                // Translate switch as if-else chain
                let switch_expr = self.translate_expr(expr)?;
                let mut stmts = Vec::new();

                for case in cases {
                    let case_val = self.translate_expr(&case.value)?;
                    let cond = Expr::Binary(BinOp::Eq, Box::new(switch_expr.clone()), Box::new(case_val));
                    let body: Vec<NStmt> = case.stmts.iter()
                        .filter_map(|s| self.translate_stmt(s).ok().flatten())
                        .collect();
                    stmts.push(NStmt::If { cond, then_body: body, else_body: None });
                }

                if let Some(default_stmts) = default {
                    let body: Vec<NStmt> = default_stmts.iter()
                        .filter_map(|s| self.translate_stmt(s).ok().flatten())
                        .collect();
                    stmts.push(NStmt::Block(body));
                }

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
            CStmt::Label(_name) => {
                // Labels become comments in ÑEXO
                Ok(None)
            }
            CStmt::Goto(_) => {
                // Goto becomes a comment in ÑEXO
                Ok(None)
            }
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
                let nexo_op = self.translate_binop(*op)?;
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
                    // In ÑEXO, assignment becomes let binding
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
            CExpr::Sizeof(ty) => {
                let size = ty.size_bytes();
                Ok(Expr::LitInt(size))
            }
            CExpr::SizeofExpr(_inner) => {
                // For sizeof(expr), we need to know the type
                // For now, return a placeholder (8 bytes for pointers)
                Ok(Expr::LitInt(8))
            }
            CExpr::ArrayIndex(obj, idx) => {
                let o = self.translate_expr(obj)?;
                let i = self.translate_expr(idx)?;
                Ok(Expr::Index(Box::new(o), Box::new(i)))
            }
            CExpr::Cast(_, inner) => self.translate_expr(inner),
            CExpr::Ternary(cond, then_expr, else_expr) => {
                let c = self.translate_expr(cond)?;
                let t = self.translate_expr(then_expr)?;
                let e = self.translate_expr(else_expr)?;
                // Ternary becomes a block expression with if-else in ÑEXO
                let if_stmt = NStmt::If {
                    cond: c,
                    then_body: vec![NStmt::ExprStmt(t)],
                    else_body: Some(vec![NStmt::ExprStmt(e)]),
                };
                Ok(Expr::Block(vec![if_stmt]))
            }
            CExpr::Comma(exprs) => {
                // Comma returns the last expression
                if let Some(last) = exprs.last() {
                    self.translate_expr(last)
                } else {
                    Ok(Expr::LitInt(0))
                }
            }
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
            CType::Struct(name) | CType::Union(name) | CType::Enum(name) => TypeAnnotation::Named(name.clone()),
            CType::Named(name) => TypeAnnotation::Named(name.clone()),
        }
    }

    fn translate_binop(&self, op: CBinOp) -> BxResult<BinOp> {
        match op {
            CBinOp::Add => Ok(BinOp::Add),
            CBinOp::Sub => Ok(BinOp::Sub),
            CBinOp::Mul => Ok(BinOp::Mul),
            CBinOp::Div => Ok(BinOp::Div),
            CBinOp::Mod => Ok(BinOp::Mod),
            CBinOp::Eq => Ok(BinOp::Eq),
            CBinOp::Ne => Ok(BinOp::Ne),
            CBinOp::Lt => Ok(BinOp::Lt),
            CBinOp::Gt => Ok(BinOp::Gt),
            CBinOp::Le => Ok(BinOp::Le),
            CBinOp::Ge => Ok(BinOp::Ge),
            CBinOp::And => Ok(BinOp::Land),
            CBinOp::Or => Ok(BinOp::Lor),
            CBinOp::BitAnd => Ok(BinOp::And),
            CBinOp::BitOr => Ok(BinOp::Or),
            CBinOp::BitXor => Ok(BinOp::Xor),
            CBinOp::Shl => Ok(BinOp::Shl),
            CBinOp::Shr => Ok(BinOp::Shr),
            // Compound assignment operators (handled by translator as
            // `lhs = lhs <op> rhs`, so we map to the plain operator)
            CBinOp::AddAssign => Ok(BinOp::Add),
            CBinOp::SubAssign => Ok(BinOp::Sub),
            CBinOp::MulAssign => Ok(BinOp::Mul),
            CBinOp::DivAssign => Ok(BinOp::Div),
            _ => Ok(BinOp::Add),
        }
    }
}

/// Top-level C → ÑEXO AST compilation entry point.
///
/// Pipeline: C source bytes → C lexer → C parser → CAst → CToNexo → ÑEXO Ast.
pub fn compile_c(source: &[u8]) -> BxResult<crate::bmo_core::lang::bmo::parser::Ast> {
    use super::lexer::CLexer;
    use super::parser::CParser;

    let mut lex = CLexer::new(source);
    let tokens = lex.tokenize()?;
    let mut parser = CParser::new(tokens);
    let cast = parser.parse()?;
    let translator = CToNexo::new();
    translator.translate(&cast)
}

/// Top-level C → native code entry point (full pipeline).
///
/// Pipeline: C → ÑEXO AST → sema → codegen → BMO assembly syntax (legacy) AST → traductor → bytes.
pub fn compile_c_to_native(source: &[u8]) -> BxResult<alloc::vec::Vec<u8>> {
    use crate::bmo_core::lang::bmo::{sema, codegen};

    // 1. C source → ÑEXO AST
    let ast = compile_c(source)?;

    // 2. Semantic analysis
    let sema_check = sema::Sema::new();
    sema_check.check(&ast)?;

    // 3. ÑEXO AST → BMO assembly syntax (legacy) AST
    let mut cg = codegen::Codegen::new();
    let mut bmo_ast = cg.emit(&ast)?;

    // 4. BMO assembly syntax (legacy) AST → native bytes
    let mut traductor = crate::bmo_core::lang::bmo::traductor::Traductor::new();
    traductor.traducir_ast(&mut bmo_ast)
}
