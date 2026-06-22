//! `lang` — Compiladores y runtimes de FastOS.
//!
//! v1.8.8: arquitectura modular estilo BMO ABI. Cada pieza es
//! autocontenida y re-usable.
//!
//! ## Capas
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │ common/   ← compartido: source, diagnostics, types, ast     │
//! │            (el "lowest common denominator" entre lenguajes) │
//! └──────▲───────────────────────────────────────────────────────┘
//!        │ usa
//! ┌──────┴───────────────────────────────────────────────────────┐
//! │ frontends/   ← N lenguajes, todos convierten a common IR     │
//! │   bmo/       (lex → parse → sema → IR)                       │
//! │   c/         (preprocessor → lex → parse → translate → IR)  │
//! │   (futuro: cpp/, rust/, java_bmo/, python_bmo/)              │
//! └──────▲───────────────────────────────────────────────────────┘
//!        │ produce common::ast::Module
//! ┌──────┴───────────────────────────────────────────────────────┐
//! │ backends/    ← genera bytes nativos                           │
//! │   aot_x86_64/   (emit, regs, abi, codegen)                  │
//! │   (futuro: aot_rdna4/, portable_ir/)                        │
//! └──────▲───────────────────────────────────────────────────────┘
//!        │ produce machine code
//! ┌──────┴───────────────────────────────────────────────────────┐
//! │ runtimes/    ← código linkeado con el binario                │
//! │   c_min/        (start, mem, string, syscall, exit)         │
//! │   (futuro: cpp_min/, java_core/, python_core/)              │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Regla de oro
//!
//! - **`common/`** puede ser usado por frontends, backends, y runtimes.
//! - **frontends/** pueden usar `common/` y entre sí (vía IR).
//! - **backends/** solo usan `common/` (operan sobre el IR).
//! - **runtimes/** solo usan `common/` y `bmo_abi/` (para syscall numbers).
//!
//! **PROHIBIDO**: frontend → backend directo, backend → frontend directo,
//! runtime → frontend, etc. Todo pasa por `common::ast`.

#![allow(dead_code)]

pub mod common;
pub mod frontends;
pub mod backends;
pub mod runtimes;
pub mod pipeline;

// `bmo/` es el módulo legacy (v1.8.7 y anterior) que contiene
// lexer/parser/sema/aot/runtime/stdlib/pm/plugins. Se mantiene
// mientras se migra al nuevo `frontends::bmo`.
pub mod bmo;

// Re-exports ergonómicos: el "API público" de lang.
pub use pipeline::{compile, CompiledProgram, SourceLang};
pub use common::{Module, Item, Stmt, Expr};
