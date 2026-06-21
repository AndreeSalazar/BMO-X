//! BMO Codegen — v1.8.0 stub.
//!
//! El pipeline BMOasm fue eliminado en v1.8.0. El codegen ahora
//! produce código nativo directamente desde el AST del parser.
//! Este módulo se mantiene como stub para compatibilidad.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use super::parser::{Ast, Stmt, Expr, BinOp, UnaryOp, TypeAnnotation};

pub struct Codegen {
    current_module: alloc::vec::Vec<alloc::string::String>,
}

impl Codegen {
    pub fn new() -> Self {
        Self { current_module: Vec::new() }
    }

    pub fn emit(&mut self, _ast: &Ast) -> BxResult<Vec<u8>> {
        Ok(Vec::new())
    }
}

impl Default for Codegen {
    fn default() -> Self { Self::new() }
}
