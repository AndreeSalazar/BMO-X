//! lang::bmo — Lenguaje de programación BMO (v1.8.0).
//!
//! Inspirado en CMD + Rust + Ada, BMO es el lenguaje nativo de FastOS.
//! Compila directamente a BMO bytecode vía `bmo::codegen`. El
//! BMOasm legacy (5,667 LOC) se eliminó en v1.8.0.
//!
//! ## Pipeline
//!
//! ```text
//!   Fuente BMO → Lexer → Parser → AST → Sema → BMO bytecode
//! ```
//!
//! ## Estado
//!
//! Fase 1: Lexer completo (32 keywords, hex/bin/oct, strings, escapes)
//! Fase 2: Parser completo (fn, let, if, while, for, struct, enum, impl, match)
//! Fase 3: Sema completo (scopes, tipos, funciones, structs)
//! Fase 4: Codegen → BMO bytecode
//! Fase 5: Pipeline end-to-end

#![allow(dead_code)]

pub mod lexer;
pub mod parser;
pub mod sema;
pub mod codegen;
pub mod modules;
pub mod runtime;
pub mod stdlib;
pub mod pm;
pub mod emit;
pub mod plugins;

// C, C++, Python, Java frontends live in plugins::languages.

use crate::bmo_gpu::BxResult;

/// Versión del lenguaje ÑEXO.
pub const NEXO_VERSION: (u8, u8, u8) = (0, 1, 0);

/// Magic bytes del bytecode ÑEXO.
pub const NEXO_MAGIC: u32 = u32::from_le_bytes(*b"NEXO");

/// Compile BMO source to native code.
///
/// Pipeline: source → lexer → parser → sema → codegen → native bytes
pub fn compile(source: &[u8]) -> BxResult<alloc::vec::Vec<u8>> {
    let mut lex = lexer::Lexer::new(source);
    let tokens = lex.tokenize()?;
    let mut parser = parser::Parser::new(&tokens);
    let ast = parser.parse()?;
    let sema = sema::Sema::new();
    sema.check(&ast)?;
    let mut codegen = codegen::Codegen::new();
    codegen.emit(&ast)
}

/// Compile C source code to native code.
#[deprecated(since = "0.9.0", note = "Use crate::bmo_core::lang::bmo::plugins::languages::c::translator::compile_c_to_native instead")]
pub fn compile_c(source: &[u8]) -> BxResult<alloc::vec::Vec<u8>> {
    crate::bmo_core::lang::bmo::plugins::languages::c::translator::compile_c_to_native(source)
}

