//! **THE SYSCALL PASS** -- validate the calls, then resolve their names.
//!
//! === Why this is a file of its own ===
//!
//! Because it is not grammar: it is a **walk over an already-built tree**, done
//! twice, for a reason that has nothing to do with C. A syscall in BMO C is a
//! name the compiler recognises, and its number comes from a table shared with
//! the kernel -- so the check is "does this call match the frozen surface", not
//! "is this valid C".
//!
//! The eight methods here are four pairs walking the same three shapes
//! (statement slice, statement, expression). That repetition reads as noise
//! inside a grammar file and as a deliberate mirror inside this one: **if a new
//! `Expr` variant is added and only three of the four walks learn about it, the
//! syscall inside it stops being seen** -- silently.
//!
//! [!] That is the failure worth guarding: an unresolved syscall does not fail
//! to compile. It compiles into a call to operation zero.

use super::*;

impl Parser {
    /// Validate that all Expr::Syscall nodes have the correct argument count.
    pub(super) fn validate_syscall_args(&self, program: &Program) -> Result<(), CError> {
        for func in &program.functions {
            Self::check_syscall_args_in_stmt_slice(&func.body, func.line)?;
        }
        Ok(())
    }

    pub(super) fn check_syscall_args_in_stmt_slice(stmts: &[Stmt], line: usize) -> Result<(), CError> {
        for stmt in stmts {
            Self::check_syscall_args_in_stmt(stmt, line)?;
        }
        Ok(())
    }

    pub(super) fn check_syscall_args_in_stmt(stmt: &Stmt, line: usize) -> Result<(), CError> {
        match stmt {
            Stmt::If(cond, t, e) => {
                Self::check_syscall_args_in_expr(cond, line)?;
                Self::check_syscall_args_in_stmt(t, line)?;
                if let Some(el) = e { Self::check_syscall_args_in_stmt(el, line)?; }
            }
            Stmt::While(cond, body) => {
                Self::check_syscall_args_in_expr(cond, line)?;
                Self::check_syscall_args_in_stmt(body, line)?;
            }
            Stmt::DoWhile(body, cond) => {
                Self::check_syscall_args_in_stmt(body, line)?;
                Self::check_syscall_args_in_expr(cond, line)?;
            }
            Stmt::For(init, cond, inc, body) => {
                if let Some(e) = init { Self::check_syscall_args_in_expr(e, line)?; }
                if let Some(e) = cond { Self::check_syscall_args_in_expr(e, line)?; }
                if let Some(e) = inc { Self::check_syscall_args_in_expr(e, line)?; }
                Self::check_syscall_args_in_stmt(body, line)?;
            }
            Stmt::Switch(expr, cases) => {
                Self::check_syscall_args_in_expr(expr, line)?;
                for c in cases { Self::check_syscall_args_in_stmt_slice(&c.stmts, line)?; }
            }
            Stmt::Block(stmts) => Self::check_syscall_args_in_stmt_slice(stmts, line)?,
            Stmt::Expr(e) | Stmt::Return(Some(e)) => Self::check_syscall_args_in_expr(e, line)?,
            Stmt::DeclAssign(_, _, Some(e)) => Self::check_syscall_args_in_expr(e, line)?,
            Stmt::DeclInit(_, _, es) => { for e in es { Self::check_syscall_args_in_expr(&e.valor, line)?; } }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn check_syscall_args_in_expr(expr: &Expr, line: usize) -> Result<(), CError> {
        match expr {
            Expr::Syscall(def, args) => {
                if args.len() != def.arg_count as usize {
                    return Err(CError::new(line, format!(
                        "syscall {}() expects {} arguments, got {}",
                        def.name, def.arg_count, args.len()
                    )));
                }
                for a in args { Self::check_syscall_args_in_expr(a, line)?; }
            }
            Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) | Expr::AddrOf(a)
                => Self::check_syscall_args_in_expr(a, line)?,
            Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b) | Expr::Mod(a,b)
                | Expr::Eq(a,b) | Expr::Neq(a,b) | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
                | Expr::BitAnd(a,b) | Expr::BitXor(a,b) | Expr::BitOr(a,b) | Expr::LAnd(a,b) | Expr::LOr(a,b)
                | Expr::Shl(a,b) | Expr::Shr(a,b) => {
                Self::check_syscall_args_in_expr(a, line)?;
                Self::check_syscall_args_in_expr(b, line)?;
            }
            Expr::Conditional(c,t,f) => {
                Self::check_syscall_args_in_expr(c, line)?;
                Self::check_syscall_args_in_expr(t, line)?;
                Self::check_syscall_args_in_expr(f, line)?;
            }
            Expr::Call(_, args) | Expr::Comma(args) => {
                for a in args { Self::check_syscall_args_in_expr(a, line)?; }
            }
            Expr::Arrow(p,_,_,_) | Expr::AssignArrow(p,_,_,_,_) => Self::check_syscall_args_in_expr(p, line)?,
            Expr::Assign(_, v) | Expr::AssignField(_,_,_,_,v) => Self::check_syscall_args_in_expr(v, line)?,
            Expr::AssignDeref(a, v) => { Self::check_syscall_args_in_expr(a, line)?; Self::check_syscall_args_in_expr(v, line)?; }
            Expr::Field(b,_,_,_) => Self::check_syscall_args_in_expr(b, line)?,
            Expr::Cast(_, a) => Self::check_syscall_args_in_expr(a, line)?,
            Expr::Intrinsic(_, args) => { for a in args { Self::check_syscall_args_in_expr(a, line)?; } }
            Expr::IndexPtr(b, idx, _) => { Self::check_syscall_args_in_expr(b, line)?; Self::check_syscall_args_in_expr(idx, line)?; }
            Expr::AssignIndexPtr(b, idx, _, v) => { Self::check_syscall_args_in_expr(b, line)?; Self::check_syscall_args_in_expr(idx, line)?; Self::check_syscall_args_in_expr(v, line)?; }
            Expr::CallPtr(c, args) => { Self::check_syscall_args_in_expr(c, line)?; for a in args { Self::check_syscall_args_in_expr(a, line)?; } }
            Expr::Subscript(_, idx, _) => Self::check_syscall_args_in_expr(idx, line)?,
            Expr::AssignSubscript(_, idx, _, v) => { Self::check_syscall_args_in_expr(idx, line)?; Self::check_syscall_args_in_expr(v, line)?; }
            Expr::AssignOp(lv, _, v) => { Self::check_syscall_args_in_expr(lv, line)?; Self::check_syscall_args_in_expr(v, line)?; }
            _ => {}
        }
        Ok(())
    }

    /// Walk all function bodies and convert Expr::Call -> Expr::Syscall for
    /// any function calls whose name matches a loaded syscall definition.
    pub(super) fn resolve_syscalls_in_program(&self, program: &mut Program) {
        for func in &mut program.functions {
            Self::resolve_syscalls_in_stmt_slice(&self.syscalls, &mut func.body);
        }
    }

    pub(super) fn resolve_syscalls_in_stmt_slice(syscalls: &HashMap<String, SyscallDef>, stmts: &mut Vec<Stmt>) {
        for stmt in stmts.iter_mut() {
            Self::resolve_syscalls_in_stmt(syscalls, stmt);
        }
    }

    pub(super) fn resolve_syscalls_in_stmt(syscalls: &HashMap<String, SyscallDef>, stmt: &mut Stmt) {
        match stmt {
            Stmt::If(cond, t, e) => {
                Self::resolve_syscalls_in_expr(syscalls, cond);
                Self::resolve_syscalls_in_stmt(syscalls, t);
                if let Some(el) = e { Self::resolve_syscalls_in_stmt(syscalls, el); }
            }
            Stmt::While(cond, body) => {
                Self::resolve_syscalls_in_expr(syscalls, cond);
                Self::resolve_syscalls_in_stmt(syscalls, body);
            }
            Stmt::DoWhile(body, cond) => {
                Self::resolve_syscalls_in_stmt(syscalls, body);
                Self::resolve_syscalls_in_expr(syscalls, cond);
            }
            Stmt::For(init, cond, inc, body) => {
                if let Some(e) = init { Self::resolve_syscalls_in_expr(syscalls, e); }
                if let Some(e) = cond { Self::resolve_syscalls_in_expr(syscalls, e); }
                if let Some(e) = inc { Self::resolve_syscalls_in_expr(syscalls, e); }
                Self::resolve_syscalls_in_stmt(syscalls, body);
            }
            Stmt::Switch(expr, cases) => {
                Self::resolve_syscalls_in_expr(syscalls, expr);
                for c in cases { Self::resolve_syscalls_in_stmt_slice(syscalls, &mut c.stmts); }
            }
            Stmt::Block(stmts) => Self::resolve_syscalls_in_stmt_slice(syscalls, stmts),
            Stmt::Expr(e) | Stmt::Return(Some(e)) => Self::resolve_syscalls_in_expr(syscalls, e),
            Stmt::DeclAssign(_, _, Some(e)) => Self::resolve_syscalls_in_expr(syscalls, e),
            Stmt::DeclInit(_, _, es) => { for e in es { Self::resolve_syscalls_in_expr(syscalls, &mut e.valor); } }
            _ => {}
        }
    }

    pub(super) fn resolve_syscalls_in_expr(syscalls: &HashMap<String, SyscallDef>, expr: &mut Expr) {
        match expr {
            Expr::Call(name, args) => {
                let mut new_args = std::mem::take(args);
                // Resolve syscalls in args first (before we potentially move them)
                for a in new_args.iter_mut() {
                    Self::resolve_syscalls_in_expr(syscalls, a);
                }
                if let Some(def) = syscalls.get(name).cloned() {
                    *expr = Expr::Syscall(def, new_args);
                } else {
                    *expr = Expr::Call(std::mem::take(name), new_args);
                }
            }
            Expr::Syscall(_, args) => {
                for a in args.iter_mut() {
                    Self::resolve_syscalls_in_expr(syscalls, a);
                }
            }
            Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) | Expr::AddrOf(a) => Self::resolve_syscalls_in_expr(syscalls, a),
            Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b) | Expr::Mod(a,b)
                | Expr::Eq(a,b) | Expr::Neq(a,b) | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
                | Expr::BitAnd(a,b) | Expr::BitXor(a,b) | Expr::BitOr(a,b) | Expr::LAnd(a,b) | Expr::LOr(a,b)
                | Expr::Shl(a,b) | Expr::Shr(a,b) => {
                Self::resolve_syscalls_in_expr(syscalls, a);
                Self::resolve_syscalls_in_expr(syscalls, b);
            }
            Expr::Conditional(c,t,f) => {
                Self::resolve_syscalls_in_expr(syscalls, c);
                Self::resolve_syscalls_in_expr(syscalls, t);
                Self::resolve_syscalls_in_expr(syscalls, f);
            }
            Expr::Arrow(p,_,_,_) | Expr::AssignArrow(p,_,_,_,_) => Self::resolve_syscalls_in_expr(syscalls, p),
            Expr::Assign(_, v) | Expr::AssignField(_,_,_,_,v) => Self::resolve_syscalls_in_expr(syscalls, v),
            Expr::AssignDeref(a, v) => { Self::resolve_syscalls_in_expr(syscalls, a); Self::resolve_syscalls_in_expr(syscalls, v); }
            Expr::Field(b,_,_,_) => Self::resolve_syscalls_in_expr(syscalls, b),
            Expr::Cast(_, a) => Self::resolve_syscalls_in_expr(syscalls, a),
            Expr::Intrinsic(_, args) => { for a in args { Self::resolve_syscalls_in_expr(syscalls, a); } }
            Expr::IndexPtr(b, idx, _) => { Self::resolve_syscalls_in_expr(syscalls, b); Self::resolve_syscalls_in_expr(syscalls, idx); }
            Expr::AssignIndexPtr(b, idx, _, v) => { Self::resolve_syscalls_in_expr(syscalls, b); Self::resolve_syscalls_in_expr(syscalls, idx); Self::resolve_syscalls_in_expr(syscalls, v); }
            Expr::CallPtr(c, args) => { Self::resolve_syscalls_in_expr(syscalls, c); for a in args { Self::resolve_syscalls_in_expr(syscalls, a); } }
            Expr::Subscript(_, idx, _) => Self::resolve_syscalls_in_expr(syscalls, idx),
            Expr::AssignSubscript(_, idx, _, v) => { Self::resolve_syscalls_in_expr(syscalls, idx); Self::resolve_syscalls_in_expr(syscalls, v); }
            Expr::AssignOp(lv, _, v) => { Self::resolve_syscalls_in_expr(syscalls, lv); Self::resolve_syscalls_in_expr(syscalls, v); }
            Expr::Comma(v) => { for e in v { Self::resolve_syscalls_in_expr(syscalls, e); } }
            _ => {}
        }
    }
}
