//! `lang::backends::aot_x86_64` — AOT compiler para x86-64.
//!
//! Es el **único backend** activo en FastOS. Compila el BMO IR
//! (`common::ast::Module`) directamente a bytes x86-64 nativos.
//!
//! ## Módulos
//!
//! - `emit`   — emite bytes x86-64 (operaciones, instrucciones, directivas)
//! - `regs`   — asignación de registros (Register Allocator)
//! - `abi`    — convención de llamada SysV AMD64 (registros, stack)
//! - `codegen` — convierte common IR a x86-64 usando los otros 3
//!
//! ## Pipeline
//!
//! ```text
//! common::ast::Module
//!   → codegen (per-function)
//!     → regs (register allocation)
//!     → emit (instruction encoding)
//!   → Vec<u8> (machine code)
//! ```
//!
//! ## Salida
//!
//! El output es un blob de bytes que contiene **una sola función**.
//! El linker (futuro) se encargará de unir múltiples funciones en un
//! BEF. Por ahora, una función = un blob.

#![allow(dead_code)]

pub mod emit;
pub mod regs;
pub mod abi;
pub mod codegen;

pub use codegen::{compile_module, compile_function};
pub use emit::Emitter;
pub use regs::RegAlloc;
