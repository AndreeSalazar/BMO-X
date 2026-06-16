//! ÑEXO Codegen — Generación de código vía BMOasm.
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

use crate::barex::BxResult;
use super::parser::{Ast, Stmt, Expr, BinOp, UnaryOp, TypeAnnotation};
use crate::lang::bmoasm::parser::ast::{Ast as BmoAst, Stmt as BmoStmt, Expr as BmoExpr, BinOp as BmoBinOp, Type as BmoType};

/// Code generator: ÑEXO AST → BMOasm AST.
pub struct Codegen {
    current_module: Vec<String>,
}

impl Codegen {
    pub fn new() -> Self {
        Self { current_module: Vec::new() }
    }

    /// Qualified name: joins module path with local name.
    fn qualified_name(&self, local_name: &str) -> String {
        if self.current_module.is_empty() {
            local_name.to_string()
        } else {
            let mut parts = self.current_module.clone();
            parts.push(local_name.to_string());
            parts.join("_")
        }
    }

    /// Generate BMOasm AST from ÑEXO AST.
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
                    if let Some(bs) = self.emit_stmt(s)? {
                        bmo_body.push(bs);
                    }
                }
                Ok(Some(BmoStmt::Def {
                    name: qualified,
                    params: bmo_params,
                    ret: bmo_ret,
                    body: bmo_body,
                }))
            }
            Stmt::Let { name, ty: _, value } => {
                let bmo_val = value.as_ref()
                    .map(|v| self.emit_expr(v))
                    .unwrap_or(Ok(BmoExpr::LitInt(0)))?;
                Ok(Some(BmoStmt::Let {
                    name: name.clone(),
                    ty: None, // BMOasm infers from value
                    value: bmo_val,
                }))
            }
            Stmt::Mut { name, ty: _, value } => {
                let bmo_val = self.emit_expr(value)?;
                Ok(Some(BmoStmt::Let {
                    name: name.clone(),
                    ty: None,
                    value: bmo_val,
                }))
            }
            Stmt::Return(Some(expr)) => {
                let bmo_val = self.emit_expr(expr)?;
                Ok(Some(BmoStmt::Retorna(Some(bmo_val))))
            }
            Stmt::Return(None) => {
                Ok(Some(BmoStmt::Retorna(None)))
            }
            Stmt::If { cond, then_body, else_body } => {
                let bmo_cond = self.emit_expr(cond)?;
                let mut bmo_then = Vec::new();
                for s in then_body {
                    if let Some(bs) = self.emit_stmt(s)? { bmo_then.push(bs); }
                }
                let bmo_else = if let Some(eb) = else_body {
                    let mut else_stmts = Vec::new();
                    for s in eb {
                        if let Some(bs) = self.emit_stmt(s)? { else_stmts.push(bs); }
                    }
                    Some(else_stmts)
                } else { None };
                Ok(Some(BmoStmt::Si { cond: bmo_cond, then_body: bmo_then, else_body: bmo_else }))
            }
            Stmt::While { cond, body } => {
                let bmo_cond = self.emit_expr(cond)?;
                let mut bmo_body = Vec::new();
                for s in body {
                    if let Some(bs) = self.emit_stmt(s)? { bmo_body.push(bs); }
                }
                Ok(Some(BmoStmt::Mientras { cond: bmo_cond, body: bmo_body }))
            }
            Stmt::Break => Ok(Some(BmoStmt::Rompe)),
            Stmt::Continue => Ok(Some(BmoStmt::Continua)),
            Stmt::Assign(name, value) => {
                let bmo_val = self.emit_expr(value)?;
                // In BMOasm, assignment is: reg rax = value; then store to variable
                // For now, use Let to rebind (BMOasm doesn't have reassignment yet)
                Ok(Some(BmoStmt::RegAssign { reg: name.clone(), value: bmo_val }))
            }
            Stmt::Block(stmts) => {
                let mut bmo_stmts = Vec::new();
                for s in stmts {
                    if let Some(bs) = self.emit_stmt(s)? { bmo_stmts.push(bs); }
                }
                // Wrap in a Def with anonymous name
                Ok(Some(BmoStmt::Def {
                    name: "_block".to_string(),
                    params: Vec::new(),
                    ret: BmoType::Void,
                    body: bmo_stmts,
                }))
            }
            Stmt::Syscall { nr, args } => {
                // Emit syscall as: reg rax = nr; [args in regs]; emit 0x0F 0x05
                let mut stmts = Vec::new();
                stmts.push(BmoStmt::RegAssign { reg: "rax".to_string(), value: BmoExpr::LitInt(*nr) });
                // Load args into registers (BMO ABI: rdi, rsi, rdx, r10, r8, r9)
                let reg_names = ["rdi", "rsi", "rdx", "r10", "r8", "r9"];
                for (i, arg) in args.iter().enumerate() {
                    if i < reg_names.len() {
                        let bmo_arg = self.emit_expr(arg)?;
                        stmts.push(BmoStmt::RegAssign { reg: reg_names[i].to_string(), value: bmo_arg });
                    }
                }
                // emit syscall instruction
                stmts.push(BmoStmt::Emit(vec![0x0F, 0x05]));
                // Return last stmt
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
            Stmt::StructDecl { .. } | Stmt::EnumDecl { .. } | Stmt::ImplDecl { .. } => {
                // Types are metadata, not codegen targets yet
                Ok(None)
            }
            Stmt::Module { name, items } => {
                // Push module path, emit items, pop
                self.current_module.push(name.clone());
                let mut results = Vec::new();
                for item in items {
                    if let Some(bs) = self.emit_stmt(item)? {
                        results.push(bs);
                    }
                }
                self.current_module.pop();
                // Return first result if only one, otherwise wrap in block
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
            Stmt::Use { .. } | Stmt::UseGlob { .. } | Stmt::Extern { .. } => {
                // Module system declarations — resolved in module resolver, not codegen
                Ok(None)
            }
            Stmt::Pub { inner } => {
                // Pub is visibility metadata — codegen the inner statement
                self.emit_stmt(inner)
            }
            Stmt::For { var, start, end, body } => {
                // Desugar: let var = start; mientras var < end { ...; var = var + 1 }
                let mut stmts = Vec::new();
                let bmo_start = self.emit_expr(start)?;
                stmts.push(BmoStmt::Let { name: var.clone(), ty: None, value: bmo_start });

                let bmo_end = self.emit_expr(end)?;
                let mut while_body = Vec::new();
                for s in body {
                    if let Some(bs) = self.emit_stmt(s)? { while_body.push(bs); }
                }
                // var = var + 1
                while_body.push(BmoStmt::RegAssign {
                    reg: var.clone(),
                    value: BmoExpr::Bin(
                        BmoBinOp::Suma,
                        Box::new(BmoExpr::Ident(var.clone())),
                        Box::new(BmoExpr::LitInt(1)),
                    ),
                });

                let cond = BmoExpr::Bin(
                    BmoBinOp::Menor,
                    Box::new(BmoExpr::Ident(var.clone())),
                    Box::new(bmo_end),
                );
                stmts.push(BmoStmt::Mientras { cond, body: while_body });

                // Wrap in a Def
                Ok(Some(BmoStmt::Def {
                    name: "_for".to_string(),
                    params: Vec::new(),
                    ret: BmoType::Void,
                    body: stmts,
                }))
            }
        }
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
                    _ => Ok(bmo_inner),
                }
            }
            Expr::Call(name, args) => {
                // Emit as: reg rax = name(args)
                let mut bmo_args = Vec::new();
                for a in args {
                    bmo_args.push(self.emit_expr(a)?);
                }
                // In BMOasm, function calls are via reg assignments
                // For now, emit as expression statement
                Ok(BmoExpr::Ident(name.clone()))
            }
            Expr::Syscall(nr, args) => {
                // Emit syscall as raw bytes
                let mut bytes = Vec::new();
                // mov rax, nr
                bytes.extend_from_slice(&[0x48, 0xB8]);
                bytes.extend_from_slice(&nr.to_le_bytes());
                // Load first arg into rdi if present
                if !args.is_empty() {
                    // For now, just emit the syscall number
                }
                bytes.extend_from_slice(&[0x0F, 0x05]); // syscall
                Ok(BmoExpr::LitInt(*nr)) // Placeholder
            }
            Expr::Emit(_bytes) => Ok(BmoExpr::LitInt(0)), // Emit bytes are statements, not expressions
            Expr::Aloc(size) => Ok(BmoExpr::Aloc(Box::new(self.emit_expr(size)?))),
            Expr::Libre(_ptr) => Ok(BmoExpr::LitNulo), // Libre is a statement
            Expr::Reg(name) => Ok(BmoExpr::Reg(name.clone())),
            Expr::Block(_stmts) => {
                // Emit block as a sequence
                Ok(BmoExpr::LitInt(0)) // Placeholder
            }
            Expr::MethodCall(_, method, _args) => {
                // Method calls become function calls
                Ok(BmoExpr::Ident(method.clone()))
            }
            Expr::Field(_obj, field) => {
                // Field access becomes offset calculation
                Ok(BmoExpr::Ident(field.clone()))
            }
            Expr::Index(_obj, _idx) => {
                // Index becomes offset calculation
                Ok(BmoExpr::LitInt(0)) // Placeholder
            }
            Expr::QualifiedPath(path) => {
                // Qualified path like `io::MAX_BUF` — flatten to single ident
                Ok(BmoExpr::Ident(path.join("::")))
            }
            Expr::QualifiedCall(path, args) => {
                // Qualified call like `io::print("hola")` — flatten path, emit first arg
                let _name = path.join("_");
                if let Some(first) = args.first() {
                    self.emit_expr(first)
                } else {
                    Ok(BmoExpr::LitInt(0))
                }
            }
        }
    }

    fn map_type(&self, ty: &TypeAnnotation) -> BmoType {
        match ty {
            TypeAnnotation::Named(name) => match name.as_str() {
                "num" | "i64" | "u64" | "i32" | "u32" | "i16" | "u16" | "i8" => BmoType::Num,
                "byte" | "u8" => BmoType::Byte,
                "bool" => BmoType::Num,
                "ptr" | "direccion" => BmoType::Ptr,
                "nulo" | "void" => BmoType::Void,
                _ => BmoType::Num, // Default to num for unknown types
            },
            TypeAnnotation::Ptr(_) => BmoType::Ptr,
            TypeAnnotation::Ref(_) => BmoType::Ptr,
            TypeAnnotation::Array(_, _) => BmoType::Arr,
            TypeAnnotation::Optional(_) => BmoType::Ptr,
            TypeAnnotation::QualifiedType(_) => BmoType::Num, // Resolved type — treat as num for now
        }
    }

    fn map_binop(&self, op: BinOp) -> BmoBinOp {
        match op {
            BinOp::Add => BmoBinOp::Suma,
            BinOp::Sub => BmoBinOp::Resta,
            BinOp::Mul => BmoBinOp::Mult,
            BinOp::Div => BmoBinOp::Div,
            BinOp::Mod => BmoBinOp::Div, // Placeholder
            BinOp::And => BmoBinOp::Y,
            BinOp::Or => BmoBinOp::O,
            BinOp::Eq => BmoBinOp::Igual,
            BinOp::Lt => BmoBinOp::Menor,
            BinOp::Gt => BmoBinOp::Mayor,
            BinOp::Le => BmoBinOp::Menor, // Placeholder
            BinOp::Ge => BmoBinOp::Mayor, // Placeholder
            BinOp::Ne => BmoBinOp::Igual, // Placeholder
            BinOp::Xor => BmoBinOp::Y, // Placeholder
            BinOp::Shl => BmoBinOp::Mult, // Placeholder
            BinOp::Shr => BmoBinOp::Div, // Placeholder
            BinOp::Land => BmoBinOp::Y,
            BinOp::Lor => BmoBinOp::O,
        }
    }
}
