//! Análisis Semántico (Sema) para BMO Simple v0.3.0.
//! Valida el AST y comprueba reglas de tipo, scopes, y asignaciones de registros.

use alloc::collections::BTreeMap;
use alloc::string::String;
use crate::bmo_gpu::{BxError, BxResult};
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
    fn_table: BTreeMap<String, usize>,
}

impl Sema {
    pub fn new() -> Self {
        Self { fn_table: BTreeMap::new() }
    }

    pub fn check(&self, ast: &Ast) -> BxResult<()> {
        for item in &ast.items {
            match item {
                Stmt::Def { name: _, params, ret, body } => {
                    let mut scope = Scope::default();
                    for (pname, pty) in params {
                        scope.push(ScopeEntry {
                            name: pname.clone(),
                            ty: pty.clone(),
                            frame_offset: 0,
                        });
                    }
                    self.check_body(body, &mut scope, ret, true, false)?;
                }
                Stmt::FnForward { .. } => {}
                Stmt::Incluye(_) => {} // Multi-file handled at traductor level
                Stmt::TypeDecl { .. } => {} // type metadata, not codegen target
                _ => return Err(BxError::InvalidArgument),
            }
        }
        Ok(())
    }

    fn check_body(
        &self,
        body: &[Stmt],
        scope: &mut Scope,
        ret_ty: &Type,
        in_fn: bool,
        in_loop: bool,
    ) -> BxResult<()> {
        for stmt in body {
            match stmt {
                Stmt::TypeDecl { .. } => {} // type metadata, not codegen target
                Stmt::Rompe | Stmt::Continua => {
                    if !in_loop {
                        return Err(BxError::InvalidArgument);
                    }
                }
                Stmt::Retorna(Some(expr)) => {
                    if !in_fn {
                        return Err(BxError::InvalidArgument);
                    }
                    let expr_ty = self.infer_type(expr, scope)?;
                    if *ret_ty != Type::Void && expr_ty != *ret_ty {
                        return Err(BxError::InvalidArgument);
                    }
                }
                Stmt::Retorna(None) => {
                    if !in_fn || *ret_ty != Type::Void {
                        return Err(BxError::InvalidArgument);
                    }
                }
                Stmt::RegAssign { reg, value } => {
                    if Reg64::from_name(reg).is_none() {
                        return Err(BxError::InvalidArgument);
                    }
                    self.check_expr(value, scope)?;
                }
                Stmt::Let { name, ty, value } => {
                    let val_ty = self.infer_type(value, scope)?;
                    // Prefer the declared type (e.g. `let p: Point`)
                    // over the inferred one — this is what carries
                    // user-defined struct names into the scope so that
                    // field access can resolve offsets.
                    let effective_ty = ty.clone().unwrap_or(val_ty);
                    if let Some(ref declared) = ty {
                        if *declared != effective_ty && *declared != Type::Void {
                            return Err(BxError::InvalidArgument);
                        }
                    }
                    if scope.lookup(name).is_some() {
                        return Err(BxError::InvalidArgument);
                    }
                    let offset = -(scope.frame_size as i32) - 8;
                    scope.frame_size += 8;
                    scope.push(ScopeEntry {
                        name: name.clone(),
                        ty: effective_ty,
                        frame_offset: offset,
                    });
                }
                Stmt::Si { cond, then_body, else_body } => {
                    self.check_expr(cond, scope)?;
                    let mut then_scope = scope.clone();
                    self.check_body(then_body, &mut then_scope, ret_ty, in_fn, in_loop)?;
                    if let Some(eb) = else_body {
                        let mut else_scope = scope.clone();
                        self.check_body(eb, &mut else_scope, ret_ty, in_fn, in_loop)?;
                    }
                }
                Stmt::Mientras { cond, body } => {
                    self.check_expr(cond, scope)?;
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
                Stmt::Barr => {}
                Stmt::Def { .. } => {
                    return Err(BxError::InvalidArgument);
                }
                Stmt::FnForward { .. } => {}
                Stmt::Incluye(_) => {}
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
                Stmt::Etiqueta(_) | Stmt::Salto(_) => {}
                Stmt::Cuando { flag: _, body } => {
                    self.check_body(body, scope, ret_ty, in_fn, in_loop)?;
                }
                Stmt::CuandoSino { flag: _, then_body, else_body } => {
                    let mut then_scope = scope.clone();
                    self.check_body(then_body, &mut then_scope, ret_ty, in_fn, in_loop)?;
                    if let Some(eb) = else_body {
                        let mut else_scope = scope.clone();
                        self.check_body(eb, &mut else_scope, ret_ty, in_fn, in_loop)?;
                    }
                }
                Stmt::Atomico(body) => {
                    self.check_body(body, scope, ret_ty, in_fn, in_loop)?;
                }
                Stmt::Volatil(expr) => {
                    self.check_expr(expr, scope)?;
                }
                Stmt::Store { value, .. } => {
                    self.check_expr(value, scope)?;
                }
                Stmt::CallStmt { args, .. } => {
                    for arg in args { self.check_expr(arg, scope)?; }
                }
                Stmt::FieldAssign { obj, value, .. } => {
                    self.check_expr(obj, scope)?;
                    self.check_expr(value, scope)?;
                }
                Stmt::IndexAssign { obj, idx, value } => {
                    self.check_expr(obj, scope)?;
                    self.check_expr(idx, scope)?;
                    self.check_expr(value, scope)?;
                }
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
            Expr::No(e) | Expr::Aloc(e) | Expr::MemOrder(_, e)
            | Expr::AddrOf(e) | Expr::Deref(e) | Expr::Cast(e, _) => {
                self.check_expr(e, scope)?;
            }
            Expr::Call { args, .. } => {
                for arg in args {
                    self.check_expr(arg, scope)?;
                }
            }
            Expr::Field { obj, .. } => self.check_expr(obj, scope)?,
            Expr::Index { obj, idx } => {
                self.check_expr(obj, scope)?;
                self.check_expr(idx, scope)?;
            }
            Expr::Reg(r_name) => {
                if r_name != "syscall" && r_name != "nop" && r_name != "pausa"
                    && r_name != "int3" && r_name != "hlt" && r_name != "cli"
                    && r_name != "sti" && r_name != "rdtsc" && r_name != "cpuid"
                    && r_name != "lfence" && r_name != "mfence" && r_name != "sfence"
                    && Reg64::from_name(r_name).is_none()
                {
                    return Err(BxError::InvalidArgument);
                }
            }
            Expr::Ident(name) => {
                if scope.lookup(name).is_none() {
                    return Err(BxError::InvalidArgument);
                }
            }
            Expr::Flag(_) => {} // CPU flags always valid
            Expr::LitInt(_) | Expr::LitByte(_) | Expr::LitNulo | Expr::LitStr(_) => {}
        }
        Ok(())
    }

    fn infer_type(&self, expr: &Expr, scope: &Scope) -> BxResult<Type> {
        match expr {
            Expr::LitInt(_) => Ok(Type::Num),
            Expr::LitByte(_) => Ok(Type::Byte),
            Expr::LitStr(_) | Expr::LitNulo => Ok(Type::Ptr),
            Expr::Ident(name) => {
                scope.lookup(name).map(|e| e.ty.clone()).ok_or(BxError::InvalidArgument)
            }
            Expr::Reg(_) | Expr::Flag(_) | Expr::Call { .. } => Ok(Type::Num),
            Expr::Bin(op, left, _) => {
                let lt = self.infer_type(left, scope)?;
                match op {
                    BinOp::Igual | BinOp::Mayor | BinOp::Menor
                    | BinOp::MayIg | BinOp::MenIg | BinOp::Difer
                    | BinOp::Y | BinOp::O => Ok(Type::Num),
                    _ => Ok(lt),
                }
            }
            Expr::No(_) => Ok(Type::Num),
            Expr::Aloc(_) => Ok(Type::Ptr),
            Expr::MemOrder(_, e) => self.infer_type(e, scope),
            Expr::AddrOf(_) => Ok(Type::Ptr),
            Expr::Deref(inner) => self.infer_type(inner, scope),
            Expr::Cast(_, t) => Ok(t.clone()),
            Expr::Field { obj, .. } => self.infer_type(obj, scope),
            Expr::Index { obj, .. } => self.infer_type(obj, scope),
        }
    }
}
