//! lang::bmo — BMO programming language (v1.8.0).
//!
//! Inspired by CMD + Rust + Ada, BMO is the native language of FastOS.
//! Compiles directly to BMO bytecode via `bmo::codegen`.
//!
//! ## Pipeline
//!
//! ```text
//!   BMO Source → Lexer → Parser → AST → Sema → BMO bytecode
//! ```
//!
//! ## Status
//!
//! Phase 1: Lexer complete (32 keywords, hex/bin/oct, strings, escapes)
//! Phase 2: Parser complete (fn, let, if, while, for, struct, enum, impl, match)
//! Phase 3: Sema complete (scopes, types, functions, structs)
//! Phase 4: Codegen → BMO bytecode
//! Phase 5: Pipeline end-to-end

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
pub mod vm;
pub mod aot;
pub mod plugins;

use crate::bmo_gpu::BxResult;

/// BMO language version.
pub const BMO_VERSION: (u8, u8, u8) = (0, 1, 0);

/// Magic bytes of BMO bytecode.
pub const BMO_MAGIC: u32 = u32::from_le_bytes(*b"BMOC");

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

/// Compile BMO source to native x86-64 machine code (AOT).
///
/// Pipeline: source → lexer → parser → sema → AOT emitter → native bytes
pub fn compile_native(source: &[u8]) -> BxResult<alloc::vec::Vec<u8>> {
    let mut lex = lexer::Lexer::new(source);
    let tokens = lex.tokenize()?;
    let mut parser = parser::Parser::new(&tokens);
    let ast = parser.parse()?;
    let sema = sema::Sema::new();
    sema.check(&ast)?;
    let mut compiler = aot::NativeCompiler::new();
    let result = compiler.compile(&ast);
    Ok(result.code)
}

/// Compile and run BMO source in one step.
///
/// Pipeline: source → compile → VM execute → return last stack value.
pub fn run(source: &[u8]) -> Result<u64, &'static str> {
    let code = compile(source).map_err(|_| "compile error")?;
    let mut vm_instance = vm::BmoVm::new();
    match vm_instance.execute(&code) {
        vm::VmExit::Halted => Ok(vm_instance.stack_top().unwrap_or(0)),
        vm::VmExit::Error(e) => Err(e),
    }
}

