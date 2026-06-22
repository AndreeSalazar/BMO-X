//! `lang::common` — Módulos compartidos por todos los frontends y backends.
//!
//! ## Capas
//!
//! ```text
//! ┌────────────────────────────────────┐
//! │ common/                            │
//! │   ├── source.rs   — Pos, Span      │ ← todos los lenguajes
//! │   ├── diagnostics.rs — errores     │
//! │   ├── types/  — IrType, IrTypeId   │ ← type system canónico
//! │   └── ast/    — BMO IR             │ ← el "lowest common denominator"
//! └────────────────────────────────────┘
//! ```
//!
//! **Regla**: cualquier cosa en `common/` puede ser usada por frontends
//! y backends. **Nada en `common/` puede depender** de un frontend
//! o backend específico (de lo contrario rompemos la modularidad).
//!
//! ## BMO IR (Intermediate Representation)
//!
//! `common::ast::Module` es el AST canónico al que **todos** los
//! lenguajes (BMO, C, Java-BMO, Python-BMO) convierten. El backend
//! AOT x86-64 opera solo sobre este AST.

#![allow(dead_code)]

pub mod source;
pub mod diagnostics;
pub mod types;
pub mod ast;

// Re-exports ergonómicos.
pub use source::{Pos, Span, FileId};
pub use diagnostics::{Diagnostic, Diagnostics, Severity, DiagCode};
pub use types::{IrType, IrTypeId, NamedTypeId, IrTypeIdList};
pub use ast::{Module, Item, Stmt, Expr, Block, Param, Field, SwitchCase, StrId,
              Linkage, TypeDeclKind, ExternKind, BinOp, UnaryOp};
