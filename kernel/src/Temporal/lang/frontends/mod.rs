//! `lang::frontends` — Todos los frontends de lenguajes soportados.
//!
//! Cada frontend es autocontenido y exporta una función `compile_to_ir`
//! que convierte source → BMO IR (`common::ast::Module`).
//!
//! ## Lenguajes soportados
//!
//! - `bmo`     — BMO nativo (lex → parse → sema → IR)
//! - `c`       — C estándar (preprocessor → lex → parse → translate → IR)
//!
//! ## Pendientes
//!
//! - `cpp`     — C++ (no implementado todavía)
//! - `rust`    — Rust (no implementado todavía)
//! - `java_bmo` — Java-BMO (no implementado)
//! - `python_bmo` — Python-BMO (no implementado)

#![allow(dead_code)]

pub mod bmo_frontend;
pub mod c;
