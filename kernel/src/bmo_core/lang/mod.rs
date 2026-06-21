//! lang — Herramientas de lenguaje de FastOS.
//!
//! Contiene los compiladores y traductores de lenguajes que operan
//! sobre la BMO ABI:
//!
//! ```text
//!   lang::bmoasm  ← Ensamblador BMO (bajo nivel, semántico-puro)
//!   lang::bmo    ← Lenguaje BMO (alto nivel, inspirado en Rust/Ada/CMD)
//! ```

#![allow(dead_code)]

pub mod bmo;
