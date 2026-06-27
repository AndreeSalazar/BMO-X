//! C Lexer — tokens específicos del lenguaje C.
//!
//! v1.8.8: re-exporta el lexer C existente de
//! `lang::bmo::plugins::languages::c::lexer`.

#![allow(dead_code)]

pub use crate::lang::bmo::plugins::languages::c::lexer::{CToken, CLexer};
pub type CTokKind = CToken;
pub type CLexError = crate::bmo_gpu::BxError;
