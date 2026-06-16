//! Constant folding — optimización en compile-time de expresiones constantes.
//! v0.3.0 — maneja todos los nodos del AST.

extern crate alloc;
use alloc::boxed::Box;
use crate::lang::bmoasm::parser::ast::{Expr, BinOp, Stmt, Ast};

pub struct Folder;

impl Folder {
    pub fn fold(ast: &mut Ast) {
        for stmt in &mut ast.items {
            Self::fold_stmt(stmt);
        }
    }

    fn fold_stmt(stmt: &mut Stmt) {
        match stmt {
            Stmt::Def { body, .. } => {
                for s in body { Self::fold_stmt(s); }
            }
            Stmt::Let { value, .. } => {
                *value = Self::fold_expr(core::mem::take(value));
            }
            Stmt::Retorna(Some(e)) => {
                *e = Self::fold_expr(core::mem::take(e));
            }
            Stmt::Si { cond, then_body, else_body } => {
                *cond = Self::fold_expr(core::mem::take(cond));
                for s in then_body { Self::fold_stmt(s); }
                if let Some(eb) = else_body {
                    for s in eb { Self::fold_stmt(s); }
                }
            }
            Stmt::Mientras { cond, body } => {
                *cond = Self::fold_expr(core::mem::take(cond));
                for s in body { Self::fold_stmt(s); }
            }
            Stmt::Match { expr, arms, default } => {
                *expr = Self::fold_expr(core::mem::take(expr));
                for (pat, body) in arms {
                    *pat = Self::fold_expr(core::mem::take(pat));
                    for s in body { Self::fold_stmt(s); }
                }
                if let Some(db) = default {
                    for s in db { Self::fold_stmt(s); }
                }
            }
            Stmt::Para { desde, hasta, paso, body, .. } => {
                *desde = Self::fold_expr(core::mem::take(desde));
                *hasta = Self::fold_expr(core::mem::take(hasta));
                if let Some(p) = paso {
                    *p = Self::fold_expr(core::mem::take(p));
                }
                for s in body { Self::fold_stmt(s); }
            }
            Stmt::Bucle(body) | Stmt::Atomico(body) => {
                for s in body { Self::fold_stmt(s); }
            }
            Stmt::ExprStmt(e) => {
                *e = Self::fold_expr(core::mem::take(e));
            }
            Stmt::RegAssign { value, .. } => {
                *value = Self::fold_expr(core::mem::take(value));
            }
            Stmt::Libre(e) | Stmt::Volatil(e) => {
                *e = Self::fold_expr(core::mem::take(e));
            }
            Stmt::Cuando { body, .. } => {
                for s in body { Self::fold_stmt(s); }
            }
            Stmt::CuandoSino { then_body, else_body, .. } => {
                for s in then_body { Self::fold_stmt(s); }
                if let Some(eb) = else_body {
                    for s in eb { Self::fold_stmt(s); }
                }
            }
            _ => {}
        }
    }

    pub fn fold_expr(expr: Expr) -> Expr {
        match expr {
            Expr::Bin(op, left, right) => {
                let l = Self::fold_expr(*left);
                let r = Self::fold_expr(*right);
                Self::fold_bin(op, l, r)
            }
            Expr::No(inner) => {
                let e = Self::fold_expr(*inner);
                match e {
                    Expr::LitInt(v) => Expr::LitInt((v == 0) as u64),
                    Expr::LitByte(v) => Expr::LitByte((v == 0) as u8),
                    other => Expr::No(Box::new(other)),
                }
            }
            Expr::Aloc(inner) => Expr::Aloc(Box::new(Self::fold_expr(*inner))),
            Expr::MemOrder(mo, inner) => Expr::MemOrder(mo, Box::new(Self::fold_expr(*inner))),
            Expr::Call { name, args } => {
                let folded = args.into_iter().map(|a| Self::fold_expr(a)).collect();
                Expr::Call { name, args: folded }
            }
            other => other,
        }
    }

    fn fold_bin(op: BinOp, left: Expr, right: Expr) -> Expr {
        match (&left, &right) {
            (Expr::LitInt(l), Expr::LitInt(r)) => {
                let result = match op {
                    BinOp::Suma  => l.wrapping_add(*r),
                    BinOp::Resta => l.wrapping_sub(*r),
                    BinOp::Mult  => l.wrapping_mul(*r),
                    BinOp::Div   => if *r != 0 { l / r } else { return Expr::Bin(op, Box::new(left), Box::new(right)); },
                    BinOp::Mod   => if *r != 0 { l % r } else { return Expr::Bin(op, Box::new(left), Box::new(right)); },
                    BinOp::Y     => l & r,
                    BinOp::O     => l | r,
                    BinOp::Xor   => l ^ r,
                    BinOp::Shl   => if *r < 64 { l << r } else { 0 },
                    BinOp::Shr   => if *r < 64 { l >> r } else { 0 },
                    BinOp::Igual  => (*l == *r) as u64,
                    BinOp::Mayor  => (*l > *r) as u64,
                    BinOp::Menor  => (*l < *r) as u64,
                    BinOp::MayIg  => (*l >= *r) as u64,
                    BinOp::MenIg  => (*l <= *r) as u64,
                    BinOp::Difer  => (*l != *r) as u64,
                };
                Expr::LitInt(result)
            }
            (Expr::LitByte(l), Expr::LitByte(r)) => {
                let result = match op {
                    BinOp::Suma  => l.wrapping_add(*r),
                    BinOp::Resta => l.wrapping_sub(*r),
                    BinOp::Mult  => l.wrapping_mul(*r),
                    BinOp::Div   => if *r != 0 { l / r } else { return Expr::Bin(op, Box::new(left), Box::new(right)); },
                    BinOp::Mod   => if *r != 0 { l % r } else { return Expr::Bin(op, Box::new(left), Box::new(right)); },
                    BinOp::Y     => l & r,
                    BinOp::O     => l | r,
                    BinOp::Xor   => l ^ r,
                    BinOp::Shl   => if (*r as u32) < 8 { l << r } else { 0 },
                    BinOp::Shr   => if (*r as u32) < 8 { l >> r } else { 0 },
                    BinOp::Igual  => (*l == *r) as u8,
                    BinOp::Mayor  => (*l > *r) as u8,
                    BinOp::Menor  => (*l < *r) as u8,
                    BinOp::MayIg  => (*l >= *r) as u8,
                    BinOp::MenIg  => (*l <= *r) as u8,
                    BinOp::Difer  => (*l != *r) as u8,
                };
                Expr::LitByte(result)
            }
            (Expr::LitInt(l), Expr::LitByte(r)) => Self::fold_bin(op, Expr::LitInt(*l), Expr::LitInt(*r as u64)),
            (Expr::LitByte(l), Expr::LitInt(r)) => Self::fold_bin(op, Expr::LitInt(*l as u64), Expr::LitInt(*r)),
            (other, Expr::LitInt(1)) => match op {
                BinOp::Mult => other.clone(),
                BinOp::Div  => other.clone(),
                BinOp::Suma => other.clone(),
                _ => Expr::Bin(op, Box::new(left), Box::new(right)),
            },
            (other, Expr::LitInt(0)) => match op {
                BinOp::Suma | BinOp::O | BinOp::Xor => other.clone(),
                BinOp::Mult | BinOp::Div | BinOp::Mod | BinOp::Shl | BinOp::Shr => Expr::LitInt(0),
                BinOp::Resta => Expr::No(Box::new(other.clone())),
                _ => Expr::Bin(op, Box::new(left), Box::new(right)),
            },
            (Expr::LitInt(0), other) => match op {
                BinOp::Suma | BinOp::Y => other.clone(),
                BinOp::Resta => Expr::No(Box::new(other.clone())),
                _ => Expr::Bin(op, Box::new(left), Box::new(right)),
            },
            _ => Expr::Bin(op, Box::new(left), Box::new(right)),
        }
    }
}
