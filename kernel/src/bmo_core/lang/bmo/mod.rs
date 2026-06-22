//! `bmo` — BMO programming language (v2.0.0).
//!
//! BMO is the **native language** of FastOS. Compiles directly to
//! **x86-64 native code** via AOT (Ahead-of-Time) — no VM, no bytecode,
//! no interpreter. The output is real machine code that runs natively on
//! the 5600X.
//!
//! ## Pipeline
//!
//! ```text
//!   BMO Source → Lexer → Parser → AST → Sema → AOT → x86-64 bytes
//! ```
//!
//! ## ABI
//!
//! BMO source code that calls BMO runtime services (windowing, FS,
//! network) goes through the **BMO ABI** syscalls (0x100..0x1FF). The
//! AOT compiler emits a real `syscall` instruction with the BMO number.
//! There is no other way to call the kernel from BMO — the BMO ABI is
//! the *only* interface.
//!
//! ## Plugin languages
//!
//! C, C++, Java, Python live in `bmo::plugins::languages::*` and are
//! integrated as `LanguageAdapter` implementations. Each adapter
//! compiles its language directly to BMO AST (or to native x86-64) and
//! the same AOT backend handles the rest.

#![allow(dead_code)]

pub mod lexer;
pub mod parser;
pub mod sema;
pub mod modules;
pub mod runtime;
pub mod stdlib;
pub mod pm;
pub mod aot;
pub mod abi;
pub mod marshal;
pub mod plugins;

use crate::bmo_gpu::BxResult;

/// BMO language version.
pub const BMO_VERSION: (u8, u8, u8) = (2, 0, 0);

/// Magic bytes for BMO AOT-compiled object (used by the BEF loader).
pub const BMO_MAGIC: u32 = u32::from_le_bytes(*b"BMOA");

/// Compile BMO source to native x86-64 machine code (AOT).
///
/// This is THE primary entry point. All BMO compilation goes through
/// here. The AOT compiler is the only backend — no VM, no bytecode,
/// no interpreter.
///
/// Pipeline: source → lexer → parser → sema → AOT → x86-64 bytes
///
/// The returned bytes are a complete x86-64 function that can be
/// called directly. Entry point is at offset 0.
pub fn compile(source: &[u8]) -> BxResult<alloc::vec::Vec<u8>> {
    compile_native(source)
}

/// Compile BMO source to native x86-64 machine code (AOT).
///
/// Same as `compile()` — kept as a separate function for API clarity
/// (matches `LanguageAdapter::compile_native`).
///
/// Pipeline: source → lexer → parser → sema → AOT → x86-64 bytes
pub fn compile_native(source: &[u8]) -> BxResult<alloc::vec::Vec<u8>> {
    let mut lex = lexer::Lexer::new(source);
    let tokens = lex.tokenize().map_err(|_| crate::bmo_gpu::BxError::InvalidArgument)?;
    let mut parser = parser::Parser::new(&tokens);
    let ast = parser.parse().map_err(|_| crate::bmo_gpu::BxError::InvalidArgument)?;
    let sema = sema::Sema::new();
    sema.check(&ast).map_err(|_| crate::bmo_gpu::BxError::InvalidArgument)?;
    let mut compiler = aot::NativeCompiler::new();
    let result = compiler.compile(&ast);
    Ok(result.code)
}

/// Lex + parse + sema only (for IDEs, syntax checkers, language
/// servers). Returns the validated AST or a BxError.
pub fn check(source: &[u8]) -> BxResult<parser::ast::Ast> {
    let mut lex = lexer::Lexer::new(source);
    let tokens = lex.tokenize()?;
    let mut parser = parser::Parser::new(&tokens);
    let ast = parser.parse()?;
    let sema = sema::Sema::new();
    sema.check(&ast)?;
    Ok(ast)
}
