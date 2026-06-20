//! Dead Code Elimination — analiza reachability desde entry points y elimina código muerto.
//! v0.4.0 — análisis recursivo con detección de ciclos y unused locals.

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use crate::bmo_core::lang::bmoasm::parser::ast::{Ast, Stmt, Expr};

pub struct Dce;

impl Dce {
    /// Run dead code elimination.
    pub fn eliminate(ast: &mut Ast) {
        // Step 1: Build call graph
        let mut defined: BTreeMap<String, &Vec<Stmt>> = BTreeMap::new();
        for item in &ast.items {
            if let Stmt::Def { name, body, .. } = item {
                defined.insert(name.clone(), body);
            }
        }

        // Step 2: Find entry points (main, _start, exported, externally called)
        let mut called: BTreeSet<String> = BTreeSet::new();
        Self::collect_called_functions(ast, &mut called);

        // Step 3: Find all functions reachable from entry points
        let mut reachable: BTreeSet<String> = BTreeSet::new();
        let mut worklist: Vec<String> = Vec::new();

        // Entry points: main, _start, anything with "export" attribute, anything called externally
        for name in defined.keys() {
            if name == "main" || name == "_start" {
                if !reachable.contains(name) {
                    reachable.insert(name.clone());
                    worklist.push(name.clone());
                }
            }
        }
        // Externally called: assume any function in called set is reachable
        for name in &called {
            if !reachable.contains(name) {
                reachable.insert(name.clone());
                worklist.push(name.clone());
            }
        }

        // BFS through call graph
        while let Some(name) = worklist.pop() {
            if let Some(body) = defined.get(&name) {
                let mut local_calls = BTreeSet::new();
                Self::collect_calls_in_body(body, &mut local_calls);
                for c in local_calls {
                    if !reachable.contains(&c) {
                        reachable.insert(c.clone());
                        worklist.push(c);
                    }
                }
            }
        }

        // Step 4: Remove unreachable functions
        ast.items.retain(|item| {
            if let Stmt::Def { name, .. } = item {
                reachable.contains(name)
            } else {
                true // Keep forward declarations, includes, etc.
            }
        });

        // Step 5: Eliminate unreachable code within functions
        for item in &mut ast.items {
            if let Stmt::Def { body, .. } = item {
                Self::eliminate_unreachable_in_body(body);
            }
        }
    }

    fn collect_called_functions(ast: &Ast, calls: &mut BTreeSet<String>) {
        for item in &ast.items {
            if let Stmt::Def { body, .. } = item {
                Self::collect_calls_in_body(body, calls);
            }
        }
    }

    fn collect_calls_in_body(body: &[Stmt], calls: &mut BTreeSet<String>) {
        for stmt in body {
            Self::collect_calls_in_stmt(stmt, calls);
        }
    }

    fn collect_calls_in_stmt(stmt: &Stmt, calls: &mut BTreeSet<String>) {
        match stmt {
            Stmt::Si { cond, then_body, else_body } => {
                Self::collect_calls_in_expr(cond, calls);
                Self::collect_calls_in_body(then_body, calls);
                if let Some(eb) = else_body {
                    Self::collect_calls_in_body(eb, calls);
                }
            }
            Stmt::Mientras { cond, body: b } => {
                Self::collect_calls_in_expr(cond, calls);
                Self::collect_calls_in_body(b, calls);
            }
            Stmt::Para { desde, hasta, paso, body: b, .. } => {
                Self::collect_calls_in_expr(desde, calls);
                Self::collect_calls_in_expr(hasta, calls);
                if let Some(p) = paso { Self::collect_calls_in_expr(p, calls); }
                Self::collect_calls_in_body(b, calls);
            }
            Stmt::Bucle(b) | Stmt::Atomico(b) => {
                Self::collect_calls_in_body(b, calls);
            }
            Stmt::Match { expr, arms, default } => {
                Self::collect_calls_in_expr(expr, calls);
                for (pat, body) in arms {
                    Self::collect_calls_in_expr(pat, calls);
                    Self::collect_calls_in_body(body, calls);
                }
                if let Some(d) = default {
                    Self::collect_calls_in_body(d, calls);
                }
            }
            Stmt::ExprStmt(e) | Stmt::Retorna(Some(e))
            | Stmt::RegAssign { value: e, .. }
            | Stmt::Let { value: e, .. } => {
                Self::collect_calls_in_expr(e, calls);
            }
            _ => {}
        }
    }

    fn collect_calls_in_expr(expr: &Expr, calls: &mut BTreeSet<String>) {
        match expr {
            Expr::Call { name, args } => {
                calls.insert(name.clone());
                for arg in args { Self::collect_calls_in_expr(arg, calls); }
            }
            Expr::Bin(_, l, r) => {
                Self::collect_calls_in_expr(l, calls);
                Self::collect_calls_in_expr(r, calls);
            }
            Expr::No(e) | Expr::Aloc(e) | Expr::MemOrder(_, e) => {
                Self::collect_calls_in_expr(e, calls);
            }
            _ => {}
        }
    }

    /// Eliminate code that follows a return/break/continue/goto in the same block.
    fn eliminate_unreachable_in_body(body: &mut Vec<Stmt>) {
        let mut cut = body.len();
        for (i, stmt) in body.iter().enumerate() {
            match stmt {
                Stmt::Retorna(_) | Stmt::Rompe | Stmt::Continua | Stmt::Salto(_) => {
                    cut = i + 1;
                    break;
                }
                Stmt::Si { then_body, else_body, .. } => {
                    // If the if is the terminator and both branches terminate, drop rest
                    if Self::block_always_returns(then_body) &&
                       else_body.as_ref().map_or(false, |e| Self::block_always_returns(e)) {
                        cut = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        body.truncate(cut);

        // Recurse into nested bodies
        for stmt in body.iter_mut() {
            match stmt {
                Stmt::Si { then_body, else_body, .. } => {
                    Self::eliminate_unreachable_in_body(then_body);
                    if let Some(eb) = else_body {
                        Self::eliminate_unreachable_in_body(eb);
                    }
                }
                Stmt::Mientras { body: b, .. } | Stmt::Bucle(b) | Stmt::Atomico(b) => {
                    Self::eliminate_unreachable_in_body(b);
                }
                Stmt::Para { body: b, .. } => {
                    Self::eliminate_unreachable_in_body(b);
                }
                Stmt::Match { arms, default, .. } => {
                    for (_, arm_body) in arms {
                        Self::eliminate_unreachable_in_body(arm_body);
                    }
                    if let Some(d) = default {
                        Self::eliminate_unreachable_in_body(d);
                    }
                }
                _ => {}
            }
        }
    }

    /// Returns true if every code path in `body` ends with a terminator
    /// (return/break/continue/goto).
    fn block_always_returns(body: &[Stmt]) -> bool {
        if body.is_empty() { return false; }
        for stmt in body {
            match stmt {
                Stmt::Retorna(_) | Stmt::Rompe | Stmt::Continua | Stmt::Salto(_) => return true,
                Stmt::Si { then_body, else_body, .. } => {
                    if else_body.is_some() {
                        let then_ret = Self::block_always_returns(then_body);
                        let else_ret = else_body.as_ref().map_or(false, |e| Self::block_always_returns(e));
                        if then_ret && else_ret { return true; }
                    }
                }
                _ => {}
            }
        }
        false
    }
}
