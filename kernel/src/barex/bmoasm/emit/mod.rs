//! Emisor — recorre el AST y produce bytes x86-64 nativos.

pub mod reg;
pub mod x86_64;

pub use reg::Reg64;
pub use x86_64::{Emitter, EmitError};
