//! Adapter: BMO AST → common IR (bmo_frontend).
//!
//! Convierte el BMO AST (producido por `lang::bmo::parser`) al BMO IR
//! canónico (`common::ast::Module`).
//!
//! ## Por qué existe
//!
//! El BMO AST legacy es muy rico (soporta todo el lenguaje BMO v2.0)
//! pero tiene su propio `TypeAnnotation`, `Expr`, `Stmt`. El common
//! IR es el "denominador común" entre frontends. Este adapter
//! abstrae el BMO AST como un caso particular.
//!
//! v1.8.8: implementa lo necesario para Hello World:
//! - Function (con body) → Item::Function
//! - Let, Assign, If, While, Return → Block
//! - IntLit, StrLit, Var, Bin, Call → Expr
//! - TypeAnnotation::Named → IrType

#![allow(dead_code)]

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::lang::common::ast as ir;
use crate::lang::common::ast::Module;
use crate::lang::common::source::Span;
use crate::lang::common::types::{IrType, IrTypeId};
use crate::lang::bmo::parser::ast::{
    Ast as BmoAst, Stmt, Expr, Param, TypeAnnotation, BinOp, UnaryOp, ExternItem,
};

/// Convierte un BMO AST al Module canónico del common IR.
pub fn lower_to_ir(ast: &BmoAst, name: &str) -> Module {
    let mut module = Module::new(name);
    // Pre-populate type table with primitives so type 0 = Void, 1 = Bool, etc.
    module.prim(IrType::Void);
    module.prim(IrType::Bool);
    module.prim(IrType::I8);
    module.prim(IrType::I16);
    module.prim(IrType::I32);
    module.prim(IrType::I64);
    module.prim(IrType::U8);
    module.prim(IrType::U16);
    module.prim(IrType::U32);
    module.prim(IrType::U64);
    module.prim(IrType::F32);
    module.prim(IrType::F64);
    module.prim(IrType::Ptr);

    for item in &ast.items {
        if let Some(ir_item) = lower_item(item, &mut module) {
            module.items.push(ir_item);
        }
    }
    module
}

fn lower_item(item: &Stmt, module: &mut Module) -> Option<ir::Item> {
    match item {
        Stmt::FnDecl { name, params, ret, body, .. } => {
            let name_id = module.intern(name);

            // Función regular (el parser legacy no tiene is_extern en FnDecl,
            // esa info viene por Stmt::Extern).
            let param_ir: Vec<ir::Param> = params.iter()
                .map(|p| ir::Param {
                    name: module.intern(&p.name),
                    ty: lower_type(&p.ty, module),
                    span: Span::ZERO,
                })
                .collect();
            let ret_ty = ret.as_ref()
                .map(|r| lower_type(r, module))
                .unwrap_or_else(|| module.prim(IrType::Void));
            let body_ir = lower_block(body, module);

            Some(ir::Item::Function {
                name: name_id,
                params: param_ir,
                ret: ret_ty,
                body: body_ir,
                linkage: ir::Linkage::External,
                span: Span::ZERO,
            })
        }
        Stmt::Pub { inner } => lower_item(inner, module),
        Stmt::Extern { items } => {
            // Tomar el primer item del extern.
            items.first().and_then(|ext| match ext {
                ExternItem::Fn { name, params, ret } => {
                    let s = module.intern(name);
                    Some(ir::Item::Extern {
                        name: s,
                        kind: ir::ExternKind::Function {
                            params: params.iter().map(|p| {
                                let _ = &p.ty;
                                IrTypeId::default()
                            }).collect(),
                            ret: ret.as_ref().map(|_| IrTypeId::default()).unwrap_or_default(),
                        },
                        span: Span::ZERO,
                    })
                }
                ExternItem::Static { name, .. } => {
                    let s = module.intern(name);
                    Some(ir::Item::Extern {
                        name: s,
                        kind: ir::ExternKind::Global { ty: IrTypeId::default() },
                        span: Span::ZERO,
                    })
                }
            })
        }
        _ => None, // Otros: ignorados por ahora
    }
}

fn lower_block(stmts: &[Stmt], module: &mut Module) -> ir::Block {
    let mut block = ir::Block::new(Span::ZERO);
    for s in stmts {
        if let Some(ir_stmt) = lower_stmt(s, module) {
            block.push(ir_stmt);
        }
    }
    block
}

fn lower_stmt(stmt: &Stmt, module: &mut Module) -> Option<ir::Stmt> {
    match stmt {
        Stmt::Let { name, ty, value, .. } => {
            let name_id = module.intern(name);
            let ty_ir = ty.as_ref().map(|t| lower_type(t, module));
            let init_ir = value.as_ref().and_then(|e| lower_expr(e, module));
            Some(ir::Stmt::Let { name: name_id, ty: ty_ir, init: init_ir, span: Span::ZERO })
        }
        Stmt::Mut { name, ty, value, .. } => {
            // Mut: igual que Let por ahora (mutabilidad se maneja en sema).
            let name_id = module.intern(name);
            let ty_ir = ty.as_ref().map(|t| lower_type(t, module));
            let init_ir = lower_expr(value, module);
            Some(ir::Stmt::Let { name: name_id, ty: ty_ir, init: init_ir, span: Span::ZERO })
        }
        Stmt::Assign(target, value) => {
            // target es un String (nombre de variable), no un Expr.
            let name_id = module.intern(target);
            let target_ir = ir::Expr::Var { name: name_id, span: Span::ZERO };
            let value_ir = lower_expr(value, module)?;
            Some(ir::Stmt::Assign { target: target_ir, value: value_ir, span: Span::ZERO })
        }
        Stmt::If { cond, then_body, else_body, .. } => {
            let cond_ir = lower_expr(cond, module)?;
            let then_ir = lower_block(then_body, module);
            let else_ir = else_body.as_ref().map(|b| lower_block(b, module));
            Some(ir::Stmt::If { cond: cond_ir, then_branch: then_ir, else_branch: else_ir, span: Span::ZERO })
        }
        Stmt::While { cond, body, .. } => {
            let cond_ir = lower_expr(cond, module)?;
            let body_ir = lower_block(body, module);
            Some(ir::Stmt::While { cond: cond_ir, body: body_ir, span: Span::ZERO })
        }
        Stmt::Return(value) => {
            let v = value.as_ref().and_then(|e| lower_expr(e, module));
            Some(ir::Stmt::Return(v, Span::ZERO))
        }
        Stmt::ExprStmt(expr) => {
            let e = lower_expr(expr, module)?;
            Some(ir::Stmt::Expr(e, Span::ZERO))
        }
        Stmt::Block(b) => {
            let body = lower_block(b, module);
            Some(ir::Stmt::Block(body))
        }
        Stmt::Break => Some(ir::Stmt::Break(Span::ZERO)),
        Stmt::Continue => Some(ir::Stmt::Continue(Span::ZERO)),
        _ => None, // Otros no soportados
    }
}

fn lower_expr(expr: &Expr, module: &mut Module) -> Option<ir::Expr> {
    match expr {
        Expr::LitInt(v) => {
            Some(ir::Expr::IntLit {
                value: *v as i128,
                ty: module.prim(IrType::I64),
                span: Span::ZERO,
            })
        }
        Expr::LitBool(value) => {
            Some(ir::Expr::BoolLit { value: *value, span: Span::ZERO })
        }
        Expr::LitStr(value) => {
            let id = module.intern(value);
            Some(ir::Expr::StrLit { id, span: Span::ZERO })
        }
        Expr::LitByte(b) => {
            Some(ir::Expr::CharLit { value: *b as u32, span: Span::ZERO })
        }
        Expr::LitNull => Some(ir::Expr::Null(Span::ZERO)),
        Expr::Ident(name) => {
            let name_id = module.intern(name);
            Some(ir::Expr::Var { name: name_id, span: Span::ZERO })
        }
        Expr::Binary(op, lhs, rhs) => {
            let l = lower_expr(lhs, module)?;
            let r = lower_expr(rhs, module)?;
            let op_ir = lower_binop(*op);
            Some(ir::Expr::Bin { op: op_ir, lhs: alloc::boxed::Box::new(l), rhs: alloc::boxed::Box::new(r), span: Span::ZERO })
        }
        Expr::Unary(op, expr) => {
            let e = lower_expr(expr, module)?;
            let op_ir = lower_unop(*op);
            Some(ir::Expr::Unary { op: op_ir, expr: alloc::boxed::Box::new(e), span: Span::ZERO })
        }
        Expr::Call(name, args) => {
            // El Call del BMO legacy es por nombre, lo modelamos como
            // Var + Call en el IR.
            let name_id = module.intern(name);
            let callee = ir::Expr::Var { name: name_id, span: Span::ZERO };
            let a: Vec<ir::Expr> = args.iter().filter_map(|e| lower_expr(e, module)).collect();
            Some(ir::Expr::Call { callee: alloc::boxed::Box::new(callee), args: a, span: Span::ZERO })
        }
        Expr::Syscall(nr, args) => {
            // Syscall directo: emitir como Call a un símbolo especial.
            // El backend lo reconocerá por el nr.
            let name = alloc::format!("__bmo_syscall_{}", nr);
            let name_id = module.intern(&name);
            let callee = ir::Expr::Var { name: name_id, span: Span::ZERO };
            let a: Vec<ir::Expr> = args.iter().filter_map(|e| lower_expr(e, module)).collect();
            Some(ir::Expr::Call { callee: alloc::boxed::Box::new(callee), args: a, span: Span::ZERO })
        }
        _ => None, // Otros: no soportados
    }
}

fn lower_type(ty: &TypeAnnotation, module: &mut Module) -> IrTypeId {
    match ty {
        TypeAnnotation::Named(n) => match n.as_str() {
            "void" | "nulo" | "()" => module.prim(IrType::Void),
            "bool" => module.prim(IrType::Bool),
            "i8" | "byte" | "i08" => module.prim(IrType::I8),
            "i16" => module.prim(IrType::I16),
            "i32" | "int" => module.prim(IrType::I32),
            "i64" | "num" | "long" | "isize" => module.prim(IrType::I64),
            "u8" | "ubyte" => module.prim(IrType::U8),
            "u16" => module.prim(IrType::U16),
            "u32" | "uint" => module.prim(IrType::U32),
            "u64" | "usize" | "size" => module.prim(IrType::U64),
            "f32" | "float" => module.prim(IrType::F32),
            "f64" | "double" => module.prim(IrType::F64),
            "str" | "string" => module.prim(IrType::Ptr),
            other => {
                let id = module.intern(other);
                module.types.intern(IrType::Named(crate::lang::common::types::NamedTypeId(id.0)))
            }
        },
        TypeAnnotation::Ptr(_) | TypeAnnotation::Ref(_) => module.prim(IrType::Ptr),
        TypeAnnotation::Array(elem, n) => {
            let e = lower_type(elem, module);
            module.types.intern(IrType::Array { elem: e, len: *n })
        }
        TypeAnnotation::Optional(inner) => lower_type(inner, module),
        _ => module.prim(IrType::Void), // Por defecto
    }
}

fn lower_binop(op: BinOp) -> ir::BinOp {
    match op {
        BinOp::Add => ir::BinOp::Add,
        BinOp::Sub => ir::BinOp::Sub,
        BinOp::Mul => ir::BinOp::Mul,
        BinOp::Div => ir::BinOp::Div,
        BinOp::Mod => ir::BinOp::Mod,
        BinOp::And => ir::BinOp::BitAnd,  // BMO And = bitwise AND
        BinOp::Or  => ir::BinOp::BitOr,   // BMO Or = bitwise OR
        BinOp::Xor => ir::BinOp::BitXor,
        BinOp::Shl => ir::BinOp::Shl,
        BinOp::Shr => ir::BinOp::Shr,
        BinOp::Land => ir::BinOp::And,    // BMO Land = logical AND
        BinOp::Lor  => ir::BinOp::Or,     // BMO Lor = logical OR
        BinOp::Eq => ir::BinOp::Eq,
        BinOp::Ne => ir::BinOp::Ne,
        BinOp::Lt => ir::BinOp::Lt,
        BinOp::Le => ir::BinOp::Le,
        BinOp::Gt => ir::BinOp::Gt,
        BinOp::Ge => ir::BinOp::Ge,
    }
}

fn lower_unop(op: UnaryOp) -> ir::UnaryOp {
    match op {
        UnaryOp::Neg => ir::UnaryOp::Neg,
        UnaryOp::Not => ir::UnaryOp::Not,
        UnaryOp::Deref => ir::UnaryOp::Not, // placeholder
        UnaryOp::Ref => ir::UnaryOp::Not,   // placeholder
    }
}
