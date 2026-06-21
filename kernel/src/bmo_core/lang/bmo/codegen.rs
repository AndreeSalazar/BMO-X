//! BMO Codegen — Code generation stub.
//!
//! Produces native code directly from the parser AST.
//! This module is maintained as a stub for compatibility.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use super::parser::Ast;

pub struct Codegen {
    current_module: Vec<alloc::string::String>,
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
