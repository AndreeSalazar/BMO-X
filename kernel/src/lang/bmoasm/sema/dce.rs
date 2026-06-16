//! Dead Code Elimination — elimina funciones nunca llamadas y código muerto.
//! v0.3.0 — análisis de reachability desde entry points.

extern crate alloc;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use crate::lang::bmoasm::parser::ast::{Ast, Stmt, Expr};

pub struct Dce;

impl Dce {
    pub fn eliminate(ast: &mut Ast) {
        // Build call graph
        let mut called: BTreeSet<String> = BTreeSet::new();
        let mut defined: BTreeSet<String> = BTreeSet::new();

        for item in &ast.items {
            if let Stmt::Def { name, body, .. } = item {
                defined.insert(name.clone());
                Self::collect_calls(body, &mut called);
            }
        }

        // Entry points: all defined functions are entry points
        // (or we could mark specific ones as entry points)
        // For now, keep everything that's defined.

        // Remove functions that are never called and are not named "main" or "_start"
        let mut keep = BTreeSet::new();
        for name in &defined {
            if called.contains(name) || name == "main" || name == "_start" {
                keep.insert(name.clone());
            }
        }

        ast.items.retain(|item| {
            if let Stmt::Def { name, .. } = item {
                keep.contains(name)
            } else {
                true
            }
        });

        // Remove unreachable statements within functions
        for item in &mut ast.items {
            if let Stmt::Def { body, .. } = item {
                Self::eliminate_unreachable(body);
            }
        }
    }

    fn collect_calls(body: &[Stmt], calls: &mut BTreeSet<String>) {
        for stmt in body {
            match stmt {
                Stmt::Si { then_body, else_body, cond } => {
                    Self::collect_calls_expr(cond, calls);
                    Self::collect_calls(then_body, calls);
                    if let Some(eb) = else_body { Self::collect_calls(eb, calls); }
                }
                Stmt::Mientras { cond, body: b } => {
                    Self::collect_calls_expr(cond, calls);
                    Self::collect_calls(b, calls);
                }
                Stmt::Para { desde, hasta, paso, body: b, .. } => {
                    Self::collect_calls_expr(desde, calls);
                    Self::collect_calls_expr(hasta, calls);
                    if let Some(p) = paso { Self::collect_calls_expr(p, calls); }
                    Self::collect_calls(b, calls);
                }
                Stmt::Bucle(b) | Stmt::Atomico(b) => {
                    Self::collect_calls(b, calls);
                }
                Stmt::Match { expr, arms, default } => {
                    Self::collect_calls_expr(expr, calls);
                    for (pat, body) in arms {
                        Self::collect_calls_expr(pat, calls);
                        Self::collect_calls(body, calls);
                    }
                    if let Some(d) = default { Self::collect_calls(d, calls); }
                }
                Stmt::ExprStmt(e) | Stmt::Retorna(Some(e))
                | Stmt::RegAssign { value: e, .. }
                | Stmt::Let { value: e, .. } => {
                    Self::collect_calls_expr(e, calls);
                }
                _ => {}
            }
        }
    }

    fn collect_calls_expr(expr: &Expr, calls: &mut BTreeSet<String>) {
        match expr {
            Expr::Call { name, args } => {
                calls.insert(name.clone());
                for arg in args { Self::collect_calls_expr(arg, calls); }
            }
            Expr::Bin(_, l, r) => {
                Self::collect_calls_expr(l, calls);
                Self::collect_calls_expr(r, calls);
            }
            Expr::No(e) | Expr::Aloc(e) | Expr::MemOrder(_, e) => {
                Self::collect_calls_expr(e, calls);
            }
            _ => {}
        }
    }

    fn eliminate_unreachable(body: &mut Vec<Stmt>) {
        let mut cut = body.len();
        for (i, stmt) in body.iter().enumerate() {
            match stmt {
                Stmt::Retorna(_) | Stmt::Rompe | Stmt::Continua | Stmt::Salto(_) => {
                    cut = i + 1;
                    break;
                }
                _ => {}
            }
        }
        body.truncate(cut);
    }
}
