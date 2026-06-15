//! lang::nexo — Lenguaje de programación ÑEXO.
//!
//! Inspirado en CMD + Rust + Ada, ÑEXO es el lenguaje nativo de FastOS/BMO.
//! Compila a BMOasm como IR intermedio, que luego emite código nativo
//! vía el emitter de BareX.
//!
//! ## Pipeline
//!
//! ```text
//!   Fuente ÑEXO → Lexer → Parser → AST → Sema → IR → BMOasm → Native
//! ```
//!
//! ## Diseño
//!
//! - **Sintaxis** inspirada en Rust (blocks, pattern matching) + Ada (contracts)
//! - **Type system** con ownership estático (sin GC)
//! - **Syscall binding** directo a la BMO ABI
//! - **Targets**: x86-64, AArch64, RISC-V (vía BMOasm emitter)
//!
//! ## Estado
//!
//! Fase 0: Skeleton — lexer + parser + AST + codegen stubs.

#![allow(dead_code)]

pub mod lexer;
pub mod parser;
pub mod sema;
pub mod codegen;

/// Versión del lenguaje ÑEXO.
pub const NEXO_VERSION: (u8, u8, u8) = (0, 1, 0);

/// Magic bytes del bytecode ÑEXO.
pub const NEXO_MAGIC: u32 = u32::from_le_bytes(*b"NEXO");
