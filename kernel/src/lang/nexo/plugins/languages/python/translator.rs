//! Python → ÑEXO Translator — converts Python AST to ÑEXO AST.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::barex::BxResult;
use super::ast::*;
use crate::lang::nexo::parser::Ast;

/// Translator stub. Full Python→ÑEXO is large; this is the entry-point.
pub struct PyToNexo;

impl PyToNexo {
    pub fn new() -> Self { Self }

    pub fn translate(&self, _past: &PyAst) -> BxResult<Ast> {
        // TODO: full Python → ÑEXO translation.
        // For now, return an empty AST so the kernel builds.
        Ok(Ast { items: Vec::new() })
    }
}
