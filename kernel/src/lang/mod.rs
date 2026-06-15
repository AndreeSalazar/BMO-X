//! lang — Herramientas de lenguaje de FastOS.
//!
//! Contiene los compiladores y traductores de lenguajes que operan
//! sobre la BMO ABI:
//!
//! ```text
//!   lang::bmoasm  ← Ensamblador BMO (bajo nivel, semántico-puro)
//!   lang::nexo    ← Lenguaje ÑEXO (alto nivel, inspirado Rust/Ada/CMD)
//! ```

#![allow(dead_code)]

pub mod bmoasm;
pub mod nexo;
