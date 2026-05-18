use crate::barex::{BxError, BxResult};
use super::super::parser::ast::Ast;

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

    /// Chequea el AST. Stub.
    pub fn check(&self, _ast: &Ast) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
