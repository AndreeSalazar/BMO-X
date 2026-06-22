//! C Lexer — tokens específicos del lenguaje C.
//!
//! v1.8.8: re-exporta el lexer C existente de `lang::bmo::plugins::languages::c`.
//! En la próxima fase se reescribirá para emitir errores con `DiagCode` canónico.

#![allow(dead_code)]

pub use crate::lang::bmo::plugins::languages::c::lexer::{CToken, CTokKind, CLexError, CLexer};
