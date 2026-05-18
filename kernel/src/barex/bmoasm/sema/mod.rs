//! Análisis semántico: scopes, resolución de identificadores, type-check.

pub mod scope;
pub mod typeck;

pub use scope::{Scope, ScopeEntry};
pub use typeck::{Sema, SemaError};
