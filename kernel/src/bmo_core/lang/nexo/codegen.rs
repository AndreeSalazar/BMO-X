//! ÑEXO Codegen — Generación de código vía BMOasm (v0.4.0).
//!
//! El codegen de ÑEXO produce AST de BMOasm como IR intermedio.
//! Luego el traductor de BMOasm lo compila a código nativo.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::string::ToString;

use crate::bmo_gpu::BxResult;
use super::parser::{Ast, Stmt, Expr, BinOp, UnaryOp, TypeAnnotation};
use crate::bmo_core::lang::bmoasm::parser::ast::{
    Ast as BmoAst, Stmt as BmoStmt, Expr as BmoExpr,
    BinOp as BmoBinOp, Type as BmoType, TypeDeclKind,
};

/// Code generator: ÑEXO AST → BMOasm AST.
pub struct Codegen {
    current_module: Vec<String>,
}

impl Codegen {
    pub fn new() -> Self {
        Self { current_module: Vec::new() }
    }

    fn qualified_name(&self, local_name: &str) -> String {
        if self.current_module.is_empty() {
            local_name.to_string()
        } else {
            let mut parts = self.current_module.clone();
            parts.push(local_name.to_string());
            parts.join("_")
        }
    }

    pub fn emit(&mut self, ast: &Ast) -> BxResult<BmoAst> {
        let mut bmo_items = Vec::new();
        for item in &ast.items {
            if let Some(stmt) = self.emit_stmt(item)? {
                bmo_items.push(stmt);
            }
        }
        Ok(BmoAst { items: bmo_items })
    }

    fn emit_stmt(&mut self, stmt: &Stmt) -> BxResult<Option<BmoStmt>> {
        match stmt {
            Stmt::FnDecl { name, params, ret, body } => {
                let qualified = self.qualified_name(name);
                let bmo_params: Vec<(String, BmoType)> = params.iter()
                    .map(|p| (p.name.clone(), self.map_type(&p.ty)))
                    .collect();
                let bmo_ret = ret.as_ref().map(|t| self.map_type(t)).unwrap_or(BmoType::Void);
                let mut bmo_body = Vec::new();
                for s in body {
                    self.emit_into(s, &mut bmo_body)?;
                }
                Ok(Some(BmoStmt::Def {
                    name: qualified,
                    params: bmo_params,
                    ret: bmo_ret,
                    body: bmo_body,
                }))
            }

            Stmt::Let { name, ty, value } => {
                let bmo_val = value.as_ref()
                    .map(|v| self.emit_expr(v))
                    .unwrap_or(Ok(BmoExpr::LitInt(0)))?;
                let bmo_ty = ty.as_ref().map(|t| self.map_type(t));
                Ok(Some(BmoStmt::Let {
                    name: name.clone(),
                    ty: bmo_ty,
                    value: bmo_val,
                }))
            }
            Stmt::Mut { name, ty, value } => {
                let bmo_val = self.emit_expr(value)?;
                let bmo_ty = ty.as_ref().map(|t| self.map_type(t));
                Ok(Some(BmoStmt::Let {
                    name: name.clone(),
                    ty: bmo_ty,
                    value: bmo_val,
                }))
            }

            Stmt::Assign(name, value) => {
                let bmo_val = self.emit_expr(value)?;
                Ok(Some(BmoStmt::Store {
                    name: name.clone(),
                    ty: None,
                    value: bmo_val,
                }))
            }

            Stmt::Return(Some(expr)) => {
                let bmo_val = self.emit_expr(expr)?;
                Ok(Some(BmoStmt::Retorna(Some(bmo_val))))
            }
            Stmt::Return(None) => Ok(Some(BmoStmt::Retorna(None))),

            Stmt::If { cond, then_body, else_body } => {
                let bmo_cond = self.emit_expr(cond)?;
                let mut bmo_then = Vec::new();
                for s in then_body { self.emit_into(s, &mut bmo_then)?; }
                let bmo_else = if let Some(eb) = else_body {
                    let mut else_stmts = Vec::new();
                    for s in eb { self.emit_into(s, &mut else_stmts)?; }
                    Some(else_stmts)
                } else { None };
                Ok(Some(BmoStmt::Si { cond: bmo_cond, then_body: bmo_then, else_body: bmo_else }))
            }

            Stmt::While { cond, body } => {
                let bmo_cond = self.emit_expr(cond)?;
                let mut bmo_body = Vec::new();
                for s in body { self.emit_into(s, &mut bmo_body)?; }
                Ok(Some(BmoStmt::Mientras { cond: bmo_cond, body: bmo_body }))
            }

            Stmt::For { var, start, end, body } => {
                let mut stmts = Vec::new();
                let bmo_start = self.emit_expr(start)?;
                stmts.push(BmoStmt::Let {
                    name: var.clone(), ty: None, value: bmo_start,
                });
                let mut while_body = Vec::new();
                for s in body { self.emit_into(s, &mut while_body)?; }
                while_body.push(BmoStmt::Store {
                    name: var.clone(), ty: None,
                    value: BmoExpr::Bin(
                        BmoBinOp::Suma,
                        Box::new(BmoExpr::Ident(var.clone())),
                        Box::new(BmoExpr::LitInt(1)),
                    ),
                });
                let bmo_end = self.emit_expr(end)?;
                let cond = BmoExpr::Bin(
                    BmoBinOp::Menor,
                    Box::new(BmoExpr::Ident(var.clone())),
                    Box::new(bmo_end),
                );
                stmts.push(BmoStmt::Mientras { cond, body: while_body });
                Ok(Some(BmoStmt::Def {
                    name: self.qualified_name("_for"),
                    params: Vec::new(),
                    ret: BmoType::Void,
                    body: stmts,
                }))
            }

            Stmt::Break => Ok(Some(BmoStmt::Rompe)),
            Stmt::Continue => Ok(Some(BmoStmt::Continua)),

            Stmt::Block(stmts) => {
                let mut bmo_stmts = Vec::new();
                for s in stmts { self.emit_into(s, &mut bmo_stmts)?; }
                Ok(Some(BmoStmt::Def {
                    name: self.qualified_name("_block"),
                    params: Vec::new(),
                    ret: BmoType::Void,
                    body: bmo_stmts,
                }))
            }

            Stmt::StructDecl { name, fields } => {
                let bmo_fields: Vec<(String, BmoType)> = fields.iter()
                    .map(|(n, t)| (n.clone(), self.map_type(t)))
                    .collect();
                Ok(Some(BmoStmt::TypeDecl {
                    name: name.clone(),
                    kind: TypeDeclKind::Struct,
                    fields: bmo_fields,
                }))
            }

            Stmt::EnumDecl { name, variants } => {
                let bmo_variants: Vec<(String, BmoType)> = variants.iter()
                    .map(|v| (v.clone(), BmoType::Num))
                    .collect();
                Ok(Some(BmoStmt::TypeDecl {
                    name: name.clone(),
                    kind: TypeDeclKind::Enum,
                    fields: bmo_variants,
                }))
            }

            Stmt::ImplDecl { type_name, methods } => {
                // Emit each method as a top-level Def with qualified name
                let mut results = Vec::new();
                for m in methods {
                    if let Some(bs) = self.emit_stmt(m)? {
                        results.push(bs);
                    }
                }
                if results.is_empty() {
                    Ok(None)
                } else if results.len() == 1 {
                    Ok(results.into_iter().next())
                } else {
                    let mut qname = self.qualified_name("impl");
                    qname.push('_');
                    qname.push_str(type_name);
                    Ok(Some(BmoStmt::Def {
                        name: qname,
                        params: Vec::new(),
                        ret: BmoType::Void,
                        body: results,
                    }))
                }
            }

            Stmt::Syscall { nr, args } => {
                let mut stmts = Vec::new();
                stmts.push(BmoStmt::RegAssign {
                    reg: "rax".to_string(),
                    value: BmoExpr::LitInt(*nr),
                });
                let reg_names = ["rdi", "rsi", "rdx", "r10", "r8", "r9"];
                for (i, arg) in args.iter().enumerate() {
                    if i < reg_names.len() {
                        let bmo_arg = self.emit_expr(arg)?;
                        stmts.push(BmoStmt::RegAssign {
                            reg: reg_names[i].to_string(),
                            value: bmo_arg,
                        });
                    }
                }
                stmts.push(BmoStmt::Emit(vec![0x0F, 0x05]));
                Ok(stmts.last().cloned())
            }

            Stmt::Emit(bytes) => Ok(Some(BmoStmt::Emit(bytes.clone()))),
            Stmt::Aloc { size } => {
                let bmo_size = self.emit_expr(size)?;
                Ok(Some(BmoStmt::ExprStmt(BmoExpr::Aloc(Box::new(bmo_size)))))
            }
            Stmt::Libre(ptr) => {
                let bmo_ptr = self.emit_expr(ptr)?;
                Ok(Some(BmoStmt::Libre(bmo_ptr)))
            }
            Stmt::ExprStmt(expr) => {
                let bmo_expr = self.emit_expr(expr)?;
                Ok(Some(BmoStmt::ExprStmt(bmo_expr)))
            }
            Stmt::Module { name, items } => {
                self.current_module.push(name.clone());
                let mut results = Vec::new();
                for item in items {
                    if let Some(bs) = self.emit_stmt(item)? {
                        results.push(bs);
                    }
                }
                self.current_module.pop();
                if results.len() == 1 {
                    Ok(results.into_iter().next())
                } else if results.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(BmoStmt::Def {
                        name: self.qualified_name(name),
                        params: Vec::new(),
                        ret: BmoType::Void,
                        body: results,
                    }))
                }
            }
            Stmt::Use { .. } | Stmt::UseGlob { .. } => {
                // Module system: resolved in module resolver, not codegen
                Ok(None)
            }
            Stmt::Extern { .. } => {
                // FFI declarations: emitted as forward decls in sema
                Ok(None)
            }
            Stmt::Pub { inner } => self.emit_stmt(inner),
        }
    }

    /// Emit a stmt into an existing body vec (helper to avoid `if let Some`)
    fn emit_into(&mut self, stmt: &Stmt, body: &mut Vec<BmoStmt>) -> BxResult<()> {
        if let Some(bs) = self.emit_stmt(stmt)? {
            body.push(bs);
        }
        Ok(())
    }

    fn emit_expr(&self, expr: &Expr) -> BxResult<BmoExpr> {
        match expr {
            Expr::LitInt(n) => Ok(BmoExpr::LitInt(*n)),
            Expr::LitFloat(b) => Ok(BmoExpr::LitInt(*b)),
            Expr::LitStr(s) => Ok(BmoExpr::LitStr(s.clone())),
            Expr::LitByte(b) => Ok(BmoExpr::LitByte(*b)),
            Expr::LitBool(true) => Ok(BmoExpr::LitInt(1)),
            Expr::LitBool(false) => Ok(BmoExpr::LitInt(0)),
            Expr::LitNull => Ok(BmoExpr::LitNulo),
            Expr::Ident(name) => Ok(BmoExpr::Ident(name.clone())),

            Expr::Binary(op, left, right) => {
                let bmo_left = self.emit_expr(left)?;
                let bmo_right = self.emit_expr(right)?;
                let bmo_op = self.map_binop(*op);
                Ok(BmoExpr::Bin(bmo_op, Box::new(bmo_left), Box::new(bmo_right)))
            }

            Expr::Unary(op, inner) => {
                let bmo_inner = self.emit_expr(inner)?;
                match op {
                    UnaryOp::Neg => Ok(BmoExpr::Bin(
                        BmoBinOp::Resta,
                        Box::new(BmoExpr::LitInt(0)),
                        Box::new(bmo_inner),
                    )),
                    UnaryOp::Not => Ok(BmoExpr::No(Box::new(bmo_inner))),
                    UnaryOp::Ref => Ok(BmoExpr::AddrOf(Box::new(bmo_inner))),
                    UnaryOp::Deref => Ok(BmoExpr::Deref(Box::new(bmo_inner))),
                }
            }

            Expr::Call(name, args) => {
                let mut bmo_args = Vec::new();
                for a in args { bmo_args.push(self.emit_expr(a)?); }
                Ok(BmoExpr::Call { name: name.clone(), args: bmo_args })
            }

            Expr::QualifiedPath(path) => {
                Ok(BmoExpr::Ident(path.join("_")))
            }

            Expr::QualifiedCall(path, args) => {
                let mut bmo_args = Vec::new();
                for a in args { bmo_args.push(self.emit_expr(a)?); }
                Ok(BmoExpr::Call {
                    name: path.join("_"),
                    args: bmo_args,
                })
            }

            Expr::MethodCall(obj, method, args) => {
                // Method dispatch: emit as call to Type_method
                let mut bmo_args = Vec::new();
                bmo_args.push(self.emit_expr(obj)?);
                for a in args { bmo_args.push(self.emit_expr(a)?); }
                Ok(BmoExpr::Call {
                    name: method.clone(),
                    args: bmo_args,
                })
            }

            Expr::Field(obj, field) => {
                let bmo_obj = self.emit_expr(obj)?;
                Ok(BmoExpr::Field {
                    obj: Box::new(bmo_obj),
                    name: field.clone(),
                })
            }

            Expr::Index(obj, idx) => {
                let bmo_obj = self.emit_expr(obj)?;
                let bmo_idx = self.emit_expr(idx)?;
                Ok(BmoExpr::Index {
                    obj: Box::new(bmo_obj),
                    idx: Box::new(bmo_idx),
                })
            }

            Expr::Syscall(nr, args) => {
                // As expression, just produce the syscall nr as a value
                let mut bmo_args = Vec::new();
                for a in args { bmo_args.push(self.emit_expr(a)?); }
                // Real codegen happens as Stmt::Syscall; this is expression
                // fallback that registers args in order via a sequence.
                // Caller should normally use Stmt::Syscall for side-effects.
                let _ = bmo_args;
                Ok(BmoExpr::LitInt(*nr))
            }

            Expr::Emit(_bytes) => Ok(BmoExpr::LitInt(0)),
            Expr::Aloc(size) => Ok(BmoExpr::Aloc(Box::new(self.emit_expr(size)?))),
            Expr::Libre(_ptr) => Ok(BmoExpr::LitNulo),
            Expr::Reg(name) => Ok(BmoExpr::Reg(name.clone())),
            Expr::Block(_stmts) => Ok(BmoExpr::LitInt(0)),
        }
    }

    fn map_type(&self, ty: &TypeAnnotation) -> BmoType {
        match ty {
            TypeAnnotation::Named(name) => match name.as_str() {
                "num" | "i64" | "u64" | "i32" | "u32" | "i16" | "u16" | "i8" | "usize" | "isize" => BmoType::Num,
                "byte" | "u8" => BmoType::Byte,
                "bool" => BmoType::Bool,
                "ptr" | "direccion" => BmoType::Ptr,
                "nulo" | "void" => BmoType::Void,
                // User-defined types: assume struct/enum, defer to resolution
                other => BmoType::Struct(other.to_string()),
            },
            TypeAnnotation::Ptr(_) => BmoType::Ptr,
            TypeAnnotation::Ref(_) => BmoType::Ref,
            TypeAnnotation::Array(_, _) => BmoType::Arr,
            TypeAnnotation::Optional(_) => BmoType::Ptr,
            TypeAnnotation::QualifiedType(path) => {
                BmoType::Struct(path.join("_"))
            }
        }
    }

    fn map_binop(&self, op: BinOp) -> BmoBinOp {
        match op {
            BinOp::Add  => BmoBinOp::Suma,
            BinOp::Sub  => BmoBinOp::Resta,
            BinOp::Mul  => BmoBinOp::Mult,
            BinOp::Div  => BmoBinOp::Div,
            BinOp::Mod  => BmoBinOp::Mod,
            BinOp::And  => BmoBinOp::Y,
            BinOp::Or   => BmoBinOp::O,
            BinOp::Xor  => BmoBinOp::Xor,
            BinOp::Shl  => BmoBinOp::Shl,
            BinOp::Shr  => BmoBinOp::Shr,
            BinOp::Eq   => BmoBinOp::Igual,
            BinOp::Ne   => BmoBinOp::Difer,
            BinOp::Lt   => BmoBinOp::Menor,
            BinOp::Gt   => BmoBinOp::Mayor,
            BinOp::Le   => BmoBinOp::MenIg,
            BinOp::Ge   => BmoBinOp::MayIg,
            BinOp::Land => BmoBinOp::Y,
            BinOp::Lor  => BmoBinOp::O,
        }
    }
}
