//! C → BMO AST translator (re-export).
//!
//! v1.8.8: re-exporta el translator C existente de
//! `lang::bmo::plugins::languages::c::translator`.

#![allow(dead_code)]

pub use crate::lang::bmo::plugins::languages::c::translator::{CToNexo, compile_c_to_native};
