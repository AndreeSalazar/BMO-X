//! Parser recursive-descent. Esqueleto — el grueso se implementará en
//! una sesión dedicada (~300 líneas).

use crate::barex::{BxError, BxResult};
use super::ast::Ast;
use super::super::lexer::Scanner;

pub struct Parser<'a> {
    pub scanner: Scanner<'a>,
}

impl<'a> Parser<'a> {
    pub const fn new(src: &'a [u8]) -> Self {
        Self { scanner: Scanner::new(src) }
    }

    /// Parsea el source completo. Stub.
    pub fn parse(&mut self) -> BxResult<Ast> {
        Err(BxError::NotImplemented)
    }
}
