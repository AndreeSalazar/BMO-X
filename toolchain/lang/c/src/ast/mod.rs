//! C Abstract Syntax Tree -- re-exports all sub-modules.

pub mod types;
pub mod expr;
pub mod stmt;
pub mod program;

// Re-export everything at the `ast::` level for backward compatibility
pub use types::*;
pub use expr::*;
pub use stmt::*;
pub use program::*;
