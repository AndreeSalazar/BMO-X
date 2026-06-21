//! C language — full frontend.
//!
//! This directory contains the complete C frontend: lexer, parser, AST,
//! and translator. The C plugin entry-point at `languages/c/plugin.rs`
//! uses `compile_c()` from the parent `bmo` module.

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod translator;
pub mod plugin;
