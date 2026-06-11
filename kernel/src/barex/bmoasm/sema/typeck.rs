//! Análisis Semántico (Sema) para BMO Simple.
//! Valida el AST y comprueba reglas de tipo y asignaciones de registros.

use crate::barex::{BxError, BxResult};
use super::super::parser::ast::{Ast, Stmt, Expr};
use super::super::emit::Reg64;

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
}

pub struct Sema;

impl Sema {
    pub const fn new() -> Self { Self }

    /// Realiza el chequeo semántico del AST.
    pub fn check(&self, ast: &Ast) -> BxResult<()> {
        for item in &ast.items {
            match item {
                Stmt::Def { name: _, params: _, ret: _, body } => {
                    self.check_body(body)?;
                }
                _ => return Err(BxError::InvalidArgument),
            }
        }
        Ok(())
    }

    fn check_body(&self, body: &[Stmt]) -> BxResult<()> {
        for stmt in body {
            match stmt {
                Stmt::RegAssign { reg, value } => {
                    // Validar nombre del registro
                    if Reg64::from_name(reg).is_none() {
                        return Err(BxError::InvalidArgument);
                    }
                    self.check_expr(value)?;
                }
                Stmt::Let { name: _, ty: _, value } => {
                    self.check_expr(value)?;
                }
                Stmt::Retorna(Some(expr)) => {
                    self.check_expr(expr)?;
                }
                Stmt::Si { cond, then_body, else_body } => {
                    self.check_expr(cond)?;
                    self.check_body(then_body)?;
                    if let Some(eb) = else_body {
                        self.check_body(eb)?;
                    }
                }
                Stmt::Mientras { cond, body } => {
                    self.check_expr(cond)?;
                    self.check_body(body)?;
                }
                Stmt::ExprStmt(expr) => {
                    self.check_expr(expr)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn check_expr(&self, expr: &Expr) -> BxResult<()> {
        match expr {
            Expr::Bin(_, left, right) => {
                self.check_expr(left)?;
                self.check_expr(right)?;
            }
            Expr::No(e) => {
                self.check_expr(e)?;
            }
            Expr::Aloc(e) => {
                self.check_expr(e)?;
            }
            Expr::Reg(r_name) => {
                // Si es un pseudo-registro (ej. 'syscall' o nombres de intrinsics), está bien.
                // De lo contrario, validar si es un registro real.
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
            _ => {}
        }
        Ok(())
    }
}
