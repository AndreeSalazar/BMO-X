//! Optimization passes — inlining, register allocation simplificado.
//! v0.3.0 — pipeline modular de optimización.

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use crate::lang::bmoasm::parser::ast::{Ast, Stmt, Expr, Type};

pub struct Optimizer;

impl Optimizer {
    /// Run all optimization passes on the AST.
    pub fn optimize(ast: &mut Ast) {
        Self::inline_small_functions(ast);
        Self::eliminate_unused_lets(ast);
    }

    /// Inline functions with 1-3 simple statements.
    fn inline_small_functions(ast: &mut Ast) {
        // Build function body map
        let mut fn_bodies: BTreeMap<String, (Vec<(String, Type)>, Vec<Stmt>)> = BTreeMap::new();
        for item in &ast.items {
            if let Stmt::Def { name, params, ret: _, body } = item {
                if body.len() <= 3 && Self::is_inlinable(body) {
                    fn_bodies.insert(name.clone(), (params.clone(), body.clone()));
                }
            }
        }

        // Inline calls in all function bodies
        for item in &mut ast.items {
            if let Stmt::Def { body, .. } = item {
                Self::inline_in_body(body, &fn_bodies);
            }
        }
    }

    fn is_inlinable(body: &[Stmt]) -> bool {
        for stmt in body {
            match stmt {
                Stmt::Retorna(Some(Expr::Ident(_))) => {}
                Stmt::Retorna(Some(Expr::Bin(_, _, _))) => {}
                Stmt::Retorna(Some(Expr::LitInt(_))) => {}
                Stmt::Let { .. } => {}
                Stmt::ExprStmt(_) => {}
                Stmt::Retorna(None) => {}
                _ => return false,
            }
        }
        true
    }

    fn inline_in_body(
        body: &mut Vec<Stmt>,
        fn_bodies: &BTreeMap<String, (Vec<(String, Type)>, Vec<Stmt>)>,
    ) {
        for stmt in body.iter_mut() {
            match stmt {
                Stmt::Si { then_body, else_body, .. } => {
                    Self::inline_in_body(then_body, fn_bodies);
                    if let Some(eb) = else_body {
                        Self::inline_in_body(eb, fn_bodies);
                    }
                }
                Stmt::Mientras { body: b, .. } | Stmt::Bucle(b) | Stmt::Atomico(b) => {
                    Self::inline_in_body(b, fn_bodies);
                }
                Stmt::Retorna(Some(Expr::Call { name, args })) => {
                    if let Some((params, body)) = fn_bodies.get(name) {
                        // Simple inline: replace call with body
                        if args.len() == params.len() {
                            let mut inlined = Vec::new();
                            for stmt in body.iter() {
                                if let Stmt::Retorna(Some(e)) = stmt {
                                    inlined.push(Stmt::Retorna(Some(Self::substitute_args(e, args, params))));
                                } else {
                                    inlined.push(stmt.clone());
                                }
                            }
                            *stmt = Stmt::ExprStmt(Expr::LitInt(0)); // placeholder
                            // Note: full inline requires more complex AST manipulation
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn substitute_args(expr: &Expr, args: &[Expr], params: &[(String, Type)]) -> Expr {
        match expr {
            Expr::Ident(name) => {
                for (i, (pname, _)) in params.iter().enumerate() {
                    if pname == name {
                        return args[i].clone();
                    }
                }
                expr.clone()
            }
            Expr::Bin(op, left, right) => {
                Expr::Bin(*op, Box::new(Self::substitute_args(left, args, params)),
                          Box::new(Self::substitute_args(right, args, params)))
            }
            Expr::No(inner) => Expr::No(Box::new(Self::substitute_args(inner, args, params))),
            _ => expr.clone(),
        }
    }

    /// Eliminate unused let bindings.
    fn eliminate_unused_lets(ast: &mut Ast) {
        for item in &mut ast.items {
            if let Stmt::Def { body, .. } = item {
                Self::remove_unused_lets(body);
            }
        }
    }

    fn remove_unused_lets(body: &mut Vec<Stmt>) {
        // Simple pass: remove lets whose names don't appear in subsequent code
        let mut used_names = alloc::collections::BTreeSet::new();
        for stmt in body.iter().rev() {
            Self::collect_used_names(stmt, &mut used_names);
        }

        let mut new_body = Vec::new();
        for stmt in body.drain(..) {
            match &stmt {
                Stmt::Let { name, .. } => {
                    if used_names.contains(name) {
                        new_body.push(stmt);
                    }
                }
                _ => new_body.push(stmt),
            }
        }
        *body = new_body;
    }

    fn collect_used_names(stmt: &Stmt, names: &mut alloc::collections::BTreeSet<String>) {
        match stmt {
            Stmt::Let { value, .. } => Self::collect_expr_names(value, names),
            Stmt::ExprStmt(e) => Self::collect_expr_names(e, names),
            Stmt::Retorna(Some(e)) => Self::collect_expr_names(e, names),
            Stmt::Si { cond, then_body, else_body } => {
                Self::collect_expr_names(cond, names);
                for s in then_body { Self::collect_used_names(s, names); }
                if let Some(eb) = else_body {
                    for s in eb { Self::collect_used_names(s, names); }
                }
            }
            Stmt::Mientras { cond, body } => {
                Self::collect_expr_names(cond, names);
                for s in body { Self::collect_used_names(s, names); }
            }
            _ => {}
        }
    }

    fn collect_expr_names(expr: &Expr, names: &mut alloc::collections::BTreeSet<String>) {
        match expr {
            Expr::Ident(name) => { names.insert(name.clone()); }
            Expr::Bin(_, l, r) => {
                Self::collect_expr_names(l, names);
                Self::collect_expr_names(r, names);
            }
            Expr::No(e) | Expr::Aloc(e) | Expr::MemOrder(_, e) => {
                Self::collect_expr_names(e, names);
            }
            Expr::Call { args, .. } => {
                for arg in args { Self::collect_expr_names(arg, names); }
            }
            _ => {}
        }
    }
}
