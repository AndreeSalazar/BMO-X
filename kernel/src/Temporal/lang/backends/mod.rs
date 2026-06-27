//! `lang::backends` — Todos los backends de generación de código.
//!
//! Cada backend toma un `common::ast::Module` y produce bytes
//! nativos del target (machine code, no bytecode).
//!
//! ## Backends
//!
//! - `aot_x86_64` — AOT x86-64 nativo (default, único activo en v1.8.8)

#![allow(dead_code)]

pub mod aot_x86_64;
