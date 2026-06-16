//! Análisis Semántico (Sema) para BMO Simple.
//! Valida el AST y comprueba reglas de tipo, scopes, y asignaciones de registros.

use alloc::collections::BTreeMap;
use alloc::string::String;
use crate::barex::{BxError, BxResult};
use super::super::parser::ast::{Ast, Stmt, Expr, Type, BinOp};
use super::super::emit::Reg64;
use super::scope::{Scope, ScopeEntry};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemaError {
    UndefinedIdent     = 1,
    TypeMismatch       = 2,
    DuplicateDef       = 3,
    InvalidRegName     = 4,
    InvalidEmitByte    = 5,
    BreakOutsideLoop   = 6,
    ReturnOutsideFn    = 7,
    UndefinedFunction  = 8,
    WrongArgCount      = 9,
}

pub struct Sema {
    /// Tabla de funciones definidas (name → param_count).
    fn_table: BTreeMap<String, usize>,
}

impl Sema {
    pub fn new() -> Self {
        Self {
            fn_table: BTreeMap::new(),
        }
    }

    /// Realiza el chequeo semántico del AST.
    pub fn check(&self, ast: &Ast) -> BxResult<()> {
        for item in &ast.items {
            match item {
                Stmt::Def { name: _, params, ret, body } => {
                    let mut scope = Scope::default();
                    for (pname, pty) in params {
                        scope.push(ScopeEntry {
                            name: pname.clone(),
                            ty: *pty,
                            frame_offset: 0,
                        });
                    }
                    self.check_body(body, &mut scope, *ret, true, false)?;
                }
                Stmt::FnForward { .. } => {
                    // Forward declarations are metadata only
                }
                _ => return Err(BxError::InvalidArgument),
            }
        }
        Ok(())
    }

    fn check_body(
        &self,
        body: &[Stmt],
        scope: &mut Scope,
        ret_ty: Type,
        in_fn: bool,
        in_loop: bool,
    ) -> BxResult<()> {
        for stmt in body {
            match stmt {
                Stmt::Rompe => {
                    if !in_loop {
                        return Err(BxError::InvalidArgument); // BreakOutsideLoop
                    }
                }
                Stmt::Continua => {
                    if !in_loop {
                        return Err(BxError::InvalidArgument); // BreakOutsideLoop (reused)
                    }
                }
                Stmt::Retorna(Some(expr)) => {
                    if !in_fn {
                        return Err(BxError::InvalidArgument); // ReturnOutsideFn
                    }
                    let expr_ty = self.infer_type(expr, scope)?;
                    if ret_ty != Type::Void && expr_ty != ret_ty {
                        return Err(BxError::InvalidArgument); // TypeMismatch
                    }
                }
                Stmt::Retorna(None) => {
                    if !in_fn {
                        return Err(BxError::InvalidArgument);
                    }
                    if ret_ty != Type::Void {
                        return Err(BxError::InvalidArgument);
                    }
                }
                Stmt::RegAssign { reg, value } => {
                    if Reg64::from_name(reg).is_none() {
                        return Err(BxError::InvalidArgument); // InvalidRegName
                    }
                    self.check_expr(value, scope)?;
                }
                Stmt::Let { name, ty, value } => {
                    let val_ty = self.infer_type(value, scope)?;
                    if let Some(declared) = ty {
                        if *declared != val_ty && *declared != Type::Void {
                            return Err(BxError::InvalidArgument); // TypeMismatch
                        }
                    }
                    if scope.lookup(name).is_some() {
                        return Err(BxError::InvalidArgument); // DuplicateDef
                    }
                    let offset = -(scope.frame_size as i32) - 8;
                    scope.frame_size += 8;
                    scope.push(ScopeEntry {
                        name: name.clone(),
                        ty: val_ty,
                        frame_offset: offset,
                    });
                }
                Stmt::Si { cond, then_body, else_body } => {
                    let cond_ty = self.infer_type(cond, scope)?;
                    if cond_ty != Type::Num && cond_ty != Type::Byte {
                        // Condition should be boolean-compatible (Num or Byte).
                    }
                    let mut then_scope = scope.clone();
                    self.check_body(then_body, &mut then_scope, ret_ty, in_fn, in_loop)?;
                    if let Some(eb) = else_body {
                        let mut else_scope = scope.clone();
                        self.check_body(eb, &mut else_scope, ret_ty, in_fn, in_loop)?;
                    }
                }
                Stmt::Mientras { cond, body } => {
                    let cond_ty = self.infer_type(cond, scope)?;
                    if cond_ty != Type::Num && cond_ty != Type::Byte {
                        // Condition should be boolean-compatible.
                    }
                    let mut loop_scope = scope.clone();
                    self.check_body(body, &mut loop_scope, ret_ty, in_fn, true)?;
                }
                Stmt::ExprStmt(expr) => {
                    self.check_expr(expr, scope)?;
                }
                Stmt::Emit(_) => {}
                Stmt::Libre(expr) => {
                    self.check_expr(expr, scope)?;
                }
                Stmt::Def { .. } => {
                    // Nested function definitions not supported.
                    return Err(BxError::InvalidArgument);
                }
                Stmt::FnForward { .. } => {}
                Stmt::Match { expr, arms, default } => {
                    self.check_expr(expr, scope)?;
                    for (pattern, body) in arms {
                        self.check_expr(pattern, scope)?;
                        let mut arm_scope = scope.clone();
                        self.check_body(body, &mut arm_scope, ret_ty, in_fn, in_loop)?;
                    }
                    if let Some(def) = default {
                        let mut def_scope = scope.clone();
                        self.check_body(def, &mut def_scope, ret_ty, in_fn, in_loop)?;
                    }
                }
                Stmt::Para { var, desde, hasta, paso, body } => {
                    self.check_expr(desde, scope)?;
                    self.check_expr(hasta, scope)?;
                    if let Some(p) = paso { self.check_expr(p, scope)?; }
                    let mut loop_scope = scope.clone();
                    loop_scope.push(ScopeEntry {
                        name: var.clone(),
                        ty: Type::Num,
                        frame_offset: 0,
                    });
                    self.check_body(body, &mut loop_scope, ret_ty, in_fn, true)?;
                }
                Stmt::Bucle(body) => {
                    self.check_body(body, scope, ret_ty, in_fn, true)?;
                }
                Stmt::Etiqueta(_) => {}
                Stmt::Salto(_) => {}
            }
        }
        Ok(())
    }

    fn check_expr(&self, expr: &Expr, scope: &Scope) -> BxResult<()> {
        match expr {
            Expr::Bin(_, left, right) => {
                self.check_expr(left, scope)?;
                self.check_expr(right, scope)?;
            }
            Expr::No(e) => {
                self.check_expr(e, scope)?;
            }
            Expr::Aloc(e) => {
                self.check_expr(e, scope)?;
            }
            Expr::Call { name: _, args } => {
                // Validate function exists (or allow forward references)
                for arg in args {
                    self.check_expr(arg, scope)?;
                }
            }
            Expr::Reg(r_name) => {
                if r_name != "syscall"
                   && r_name != "nop"
                   && r_name != "pausa"
                   && r_name != "int3"
                   && r_name != "hlt"
                   && r_name != "cli"
                   && r_name != "sti"
                   && r_name != "rdtsc"
                   && r_name != "cpuid"
                   && r_name != "lfence"
                   && r_name != "mfence"
                   && r_name != "sfence"
                   && Reg64::from_name(r_name).is_none()
                {
                    return Err(BxError::InvalidArgument);
                }
            }
            Expr::Ident(name) => {
                if scope.lookup(name).is_none() {
                    return Err(BxError::InvalidArgument); // UndefinedIdent
                }
            }
            Expr::LitInt(_) | Expr::LitByte(_) | Expr::LitNulo | Expr::LitStr(_) => {}
        }
        Ok(())
    }

    fn infer_type(&self, expr: &Expr, scope: &Scope) -> BxResult<Type> {
        match expr {
            Expr::LitInt(_) => Ok(Type::Num),
            Expr::LitByte(_) => Ok(Type::Byte),
            Expr::LitStr(_) => Ok(Type::Ptr),
            Expr::LitNulo => Ok(Type::Ptr),
            Expr::Ident(name) => {
                scope.lookup(name)
                    .map(|e| e.ty)
                    .ok_or(BxError::InvalidArgument) // UndefinedIdent
            }
            Expr::Reg(_) => Ok(Type::Num),
            Expr::Call { .. } => Ok(Type::Num), // Default return type
            Expr::Bin(op, left, _right) => {
                let lt = self.infer_type(left, scope)?;
                match op {
                    BinOp::Igual | BinOp::Mayor | BinOp::Menor | BinOp::Y | BinOp::O => Ok(Type::Num),
                    _ => Ok(lt), // Arithmetic preserves left operand type
                }
            }
            Expr::No(_) => Ok(Type::Num),
            Expr::Aloc(_) => Ok(Type::Ptr),
        }
    }
}
