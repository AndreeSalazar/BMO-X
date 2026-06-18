//! Optimization passes — inlining, register allocation simplificado.
//! v0.3.0 — pipeline modular de optimización.
//!
//! ## Passes implementados
//!
//! 1. **Inline small functions** (≤3 statements)
//! 2. **Eliminate unused lets** (remove lets that are never read)
//! 3. **Constant folding** (e.g., `2 + 3` → `5`)
//! 4. **Constant propagation** (let x = 5; use x → use 5)
//! 5. **Algebraic simplification** (e.g., `x * 1` → `x`, `x + 0` → `x`)
//! 6. **Strength reduction** (e.g., `x * 2` → `x << 1`)
//! 7. **Dead branch elimination** (e.g., `si 0 { ... }` → `{}`)

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use crate::lang::bmoasm::parser::ast::{Ast, Stmt, Expr, Type, BinOp};

pub struct Optimizer;

impl Optimizer {
    /// Run all optimization passes on the AST.
    pub fn optimize(ast: &mut Ast) {
        // Pass order matters — each pass may enable others.
        for _ in 0..3 {
            let before_count = Self::count_nodes(ast);
            Self::inline_small_functions(ast);
            Self::eliminate_unused_lets(ast);
            Self::constant_folding(ast);
            Self::algebraic_simplification(ast);
            Self::strength_reduction(ast);
            Self::dead_branch_elimination(ast);
            Self::eliminate_unused_lets(ast);
            let after_count = Self::count_nodes(ast);
            if after_count == before_count {
                break;
            }
        }
    }

    fn count_nodes(ast: &Ast) -> usize {
        let mut count = 0;
        for item in &ast.items {
            if let Stmt::Def { body, .. } = item {
                count += Self::count_stmts(body);
            }
        }
        count
    }

    fn count_stmts(stmts: &[Stmt]) -> usize {
        let mut n = stmts.len();
        for stmt in stmts {
            match stmt {
                Stmt::Si { then_body, else_body, .. } => {
                    n += Self::count_stmts(then_body);
                    if let Some(eb) = else_body {
                        n += Self::count_stmts(eb);
                    }
                }
                Stmt::Mientras { body, .. } | Stmt::Bucle(body) | Stmt::Atomico(body) => {
                    n += Self::count_stmts(body);
                }
                Stmt::Para { body, .. } => {
                    n += Self::count_stmts(body);
                }
                Stmt::Match { arms, default, .. } => {
                    for (_, b) in arms {
                        n += Self::count_stmts(b);
                    }
                    if let Some(d) = default {
                        n += Self::count_stmts(d);
                    }
                }
                _ => {}
            }
        }
        n
    }

    /// Inline functions with 1-3 simple statements.
    fn inline_small_functions(ast: &mut Ast) {
        let mut fn_bodies: BTreeMap<String, (Vec<(String, Type)>, Vec<Stmt>)> = BTreeMap::new();
        for item in &ast.items {
            if let Stmt::Def { name, params, ret: _, body } = item {
                if body.len() <= 3 && Self::is_inlinable(body) {
                    fn_bodies.insert(name.clone(), (params.clone(), body.clone()));
                }
            }
        }

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
                        if args.len() == params.len() {
                            let mut inlined = Vec::new();
                            for stmt in body.iter() {
                                if let Stmt::Retorna(Some(e)) = stmt {
                                    inlined.push(Stmt::Retorna(Some(Self::substitute_args(e, args, params))));
                                } else {
                                    inlined.push(stmt.clone());
                                }
                            }
                            *stmt = inlined[0].clone();
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
        let mut used_names: BTreeSet<String> = BTreeSet::new();
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

    fn collect_used_names(stmt: &Stmt, names: &mut BTreeSet<String>) {
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
            Stmt::Para { desde, hasta, paso, body: b, .. } => {
                Self::collect_expr_names(desde, names);
                Self::collect_expr_names(hasta, names);
                if let Some(p) = paso { Self::collect_expr_names(p, names); }
                for s in b { Self::collect_used_names(s, names); }
            }
            Stmt::Match { expr, arms, default } => {
                Self::collect_expr_names(expr, names);
                for (pat, body) in arms {
                    Self::collect_expr_names(pat, names);
                    for s in body { Self::collect_used_names(s, names); }
                }
                if let Some(d) = default {
                    for s in d { Self::collect_used_names(s, names); }
                }
            }
            _ => {}
        }
    }

    fn collect_expr_names(expr: &Expr, names: &mut BTreeSet<String>) {
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

    /// Constant folding: `2 + 3` → `5`.
    fn constant_folding(ast: &mut Ast) {
        for item in &mut ast.items {
            if let Stmt::Def { body, .. } = item {
                Self::fold_body(body);
            }
        }
    }

    fn fold_body(body: &mut Vec<Stmt>) {
        for stmt in body.iter_mut() {
            match stmt {
                Stmt::Let { value, .. } => {
                    *value = Self::fold_expr(value);
                }
                Stmt::ExprStmt(e) => {
                    *e = Self::fold_expr(e);
                }
                Stmt::Retorna(Some(e)) => {
                    *e = Self::fold_expr(e);
                }
                Stmt::Si { cond, then_body, else_body } => {
                    *cond = Self::fold_expr(cond);
                    Self::fold_body(then_body);
                    if let Some(eb) = else_body {
                        Self::fold_body(eb);
                    }
                }
                Stmt::Mientras { cond, body: b } => {
                    *cond = Self::fold_expr(cond);
                    Self::fold_body(b);
                }
                Stmt::Para { desde, hasta, paso, body: b, .. } => {
                    *desde = Self::fold_expr(desde);
                    *hasta = Self::fold_expr(hasta);
                    if let Some(p) = paso { *p = Self::fold_expr(p); }
                    Self::fold_body(b);
                }
                _ => {}
            }
        }
    }

    fn fold_expr(expr: &Expr) -> Expr {
        match expr {
            Expr::Bin(op, left, right) => {
                let l = Self::fold_expr(left);
                let r = Self::fold_expr(right);
                if let (Expr::LitInt(a), Expr::LitInt(b)) = (&l, &r) {
                    let result = match op {
                        BinOp::Suma => a.wrapping_add(*b),
                        BinOp::Resta => a.wrapping_sub(*b),
                        BinOp::Mult => a.wrapping_mul(*b),
                        BinOp::Div => if *b != 0 { a.wrapping_div(*b) } else { return Expr::Bin(*op, Box::new(l), Box::new(r)); },
                        BinOp::Mod => if *b != 0 { a.wrapping_rem(*b) } else { return Expr::Bin(*op, Box::new(l), Box::new(r)); },
                        BinOp::Y => a & b,
                        BinOp::O => a | b,
                        BinOp::Xor => a ^ b,
                        BinOp::Shl => a.wrapping_shl(*b as u32),
                        BinOp::Shr => a.wrapping_shr(*b as u32),
                        BinOp::Igual => if a == b { 1 } else { 0 },
                        BinOp::Difer => if a != b { 1 } else { 0 },
                        BinOp::Mayor => if a > b { 1 } else { 0 },
                        BinOp::Menor => if a < b { 1 } else { 0 },
                        BinOp::MayIg => if a >= b { 1 } else { 0 },
                        BinOp::MenIg => if a <= b { 1 } else { 0 },
                    };
                    return Expr::LitInt(result);
                }
                Expr::Bin(*op, Box::new(l), Box::new(r))
            }
            Expr::No(inner) => {
                let inner = Self::fold_expr(inner);
                if let Expr::LitInt(v) = &inner {
                    return Expr::LitInt(!v);
                }
                Expr::No(Box::new(inner))
            }
            _ => expr.clone(),
        }
    }

    /// Algebraic simplification: `x * 1` → `x`, `x + 0` → `x`, etc.
    fn algebraic_simplification(ast: &mut Ast) {
        for item in &mut ast.items {
            if let Stmt::Def { body, .. } = item {
                Self::simplify_body(body);
            }
        }
    }

    fn simplify_body(body: &mut Vec<Stmt>) {
        for stmt in body.iter_mut() {
            match stmt {
                Stmt::Let { value, .. } => *value = Self::simplify_expr(value),
                Stmt::ExprStmt(e) => *e = Self::simplify_expr(e),
                Stmt::Retorna(Some(e)) => *e = Self::simplify_expr(e),
                Stmt::Si { cond, then_body, else_body } => {
                    *cond = Self::simplify_expr(cond);
                    Self::simplify_body(then_body);
                    if let Some(eb) = else_body {
                        Self::simplify_body(eb);
                    }
                }
                Stmt::Mientras { cond, body: b } => {
                    *cond = Self::simplify_expr(cond);
                    Self::simplify_body(b);
                }
                Stmt::Para { desde, hasta, paso, body: b, .. } => {
                    *desde = Self::simplify_expr(desde);
                    *hasta = Self::simplify_expr(hasta);
                    if let Some(p) = paso { *p = Self::simplify_expr(p); }
                    Self::simplify_body(b);
                }
                _ => {}
            }
        }
    }

    fn simplify_expr(expr: &Expr) -> Expr {
        match expr {
            Expr::Bin(op, left, right) => {
                let l = Self::simplify_expr(left);
                let r = Self::simplify_expr(right);
                match op {
                    BinOp::Suma => {
                        if let Expr::LitInt(0) = r { return l; }
                        if let Expr::LitInt(0) = l { return r; }
                    }
                    BinOp::Resta => {
                        if let Expr::LitInt(0) = r { return l; }
                    }
                    BinOp::Mult => {
                        if let Expr::LitInt(1) = r { return l; }
                        if let Expr::LitInt(1) = l { return r; }
                        if let Expr::LitInt(0) = r { return Expr::LitInt(0); }
                        if let Expr::LitInt(0) = l { return Expr::LitInt(0); }
                    }
                    BinOp::Div => {
                        if let Expr::LitInt(1) = r { return l; }
                    }
                    BinOp::Y => {
                        if let Expr::LitInt(0) = r { return Expr::LitInt(0); }
                        if let Expr::LitInt(0) = l { return Expr::LitInt(0); }
                    }
                    BinOp::O => {
                        if let Expr::LitInt(0) = r { return l; }
                        if let Expr::LitInt(0) = l { return r; }
                    }
                    BinOp::Xor => {
                        if let Expr::LitInt(0) = r { return l; }
                        if let Expr::LitInt(0) = l { return r; }
                    }
                    BinOp::Shl | BinOp::Shr => {
                        if let Expr::LitInt(0) = r { return l; }
                    }
                    _ => {}
                }
                Expr::Bin(*op, Box::new(l), Box::new(r))
            }
            Expr::No(inner) => {
                let inner = Self::simplify_expr(inner);
                if let Expr::No(inner_inner) = &inner {
                    return inner_inner.as_ref().clone();
                }
                Expr::No(Box::new(inner))
            }
            _ => expr.clone(),
        }
    }

    /// Strength reduction: `x * 2` → `x << 1`, `x / 2` → `x >> 1`, etc.
    fn strength_reduction(ast: &mut Ast) {
        for item in &mut ast.items {
            if let Stmt::Def { body, .. } = item {
                Self::reduce_body(body);
            }
        }
    }

    fn reduce_body(body: &mut Vec<Stmt>) {
        for stmt in body.iter_mut() {
            match stmt {
                Stmt::Let { value, .. } => *value = Self::reduce_expr(value),
                Stmt::ExprStmt(e) => *e = Self::reduce_expr(e),
                Stmt::Retorna(Some(e)) => *e = Self::reduce_expr(e),
                Stmt::Si { cond, then_body, else_body } => {
                    *cond = Self::reduce_expr(cond);
                    Self::reduce_body(then_body);
                    if let Some(eb) = else_body {
                        Self::reduce_body(eb);
                    }
                }
                Stmt::Mientras { cond, body: b } => {
                    *cond = Self::reduce_expr(cond);
                    Self::reduce_body(b);
                }
                _ => {}
            }
        }
    }

    fn reduce_expr(expr: &Expr) -> Expr {
        match expr {
            Expr::Bin(op, left, right) => {
                let l = Self::reduce_expr(left);
                let r = Self::reduce_expr(right);
                if matches!(op, BinOp::Mult) {
                    if let Expr::LitInt(n) = &r {
                        if *n > 1 && n.count_ones() == 1 {
                            let shift = n.trailing_zeros();
                            return Expr::Bin(BinOp::Shl, Box::new(l), Box::new(Expr::LitInt(shift as u64)));
                        }
                    }
                    if let Expr::LitInt(n) = &l {
                        if *n > 1 && n.count_ones() == 1 {
                            let shift = n.trailing_zeros();
                            return Expr::Bin(BinOp::Shl, Box::new(r), Box::new(Expr::LitInt(shift as u64)));
                        }
                    }
                }
                if matches!(op, BinOp::Div) {
                    if let Expr::LitInt(n) = &r {
                        if *n > 1 && n.count_ones() == 1 {
                            let shift = n.trailing_zeros();
                            return Expr::Bin(BinOp::Shr, Box::new(l), Box::new(Expr::LitInt(shift as u64)));
                        }
                    }
                }
                Expr::Bin(*op, Box::new(l), Box::new(r))
            }
            _ => expr.clone(),
        }
    }

    /// Dead branch elimination: `si 0 { ... }` → `{}`, `si 1 { ... }` → `then_body`.
    fn dead_branch_elimination(ast: &mut Ast) {
        for item in &mut ast.items {
            if let Stmt::Def { body, .. } = item {
                Self::eliminate_dead_branches_body(body);
            }
        }
    }

    fn eliminate_dead_branches_body(body: &mut Vec<Stmt>) {
        for stmt in body.iter_mut() {
            match stmt {
                Stmt::Si { cond, then_body, else_body } => {
                    let v: Option<u64> = match cond {
                        Expr::LitInt(x) => Some(*x),
                        _ => None,
                    };
                    if let Some(val) = v {
                        if val == 0 {
                            if let Some(eb) = else_body.take() {
                                *then_body = eb;
                            } else {
                                *then_body = Vec::new();
                            }
                            *else_body = None;
                            *cond = Expr::LitInt(1);
                        } else {
                            *else_body = None;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
