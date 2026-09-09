//! C Abstract Syntax Tree -- re-exports all sub-modules.
//!
//! [fase]     ARBOL
//!
//! [aparece]  AQUI -- es una fachada de re-exportacion: si falta algo, no
//!            compila el compilador
//!
//! [carril]   VERDE    -- si se rompe, ALGUIEN TE LO DICE antes de que salga de aqui
//!            * y sale de su `[aparece]`, no de una opinion: ver toolchain/tools/fases/
//!

pub mod types;
pub mod expr;
pub mod stmt;
pub mod program;

// Re-export everything at the `ast::` level for backward compatibility
pub use types::*;
pub use expr::*;
pub use stmt::*;
pub use program::*;
