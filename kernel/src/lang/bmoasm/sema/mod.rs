//! Análisis semántico: scopes, resolución de identificadores, type-check, constant folding.

pub mod scope;
pub mod typeck;
pub mod fold;

pub use typeck::Sema;
