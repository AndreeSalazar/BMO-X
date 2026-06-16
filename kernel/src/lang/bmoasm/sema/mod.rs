//! Análisis semántico: scopes, resolución de identificadores, type-check,
//! constant folding, dead code elimination, inlining.

pub mod scope;
pub mod typeck;
pub mod fold;
pub mod dce;
pub mod opt;

pub use typeck::Sema;
