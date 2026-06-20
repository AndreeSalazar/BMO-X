//! Language plugin implementations for ÑEXO.
//!
//! Each language provides a complete plugin implementation:
//! - `c/`     - C language plugin (full frontend)
//! - `cpp/`   - C++ language plugin (essential subset)
//! - `rust/`  - Rust language plugin
//! - `go/`    - Go language plugin
//! - `python/`- Python language plugin
//! - `java/`  - Java language plugin (planned)
//!
//! Each language lives in its own subdirectory so it can have its own
//! lexer, parser, AST, translator, and tests. They all produce ÑEXO AST
//! which then goes through codegen → BMOasm → native.

#![allow(dead_code)]

pub mod c;
pub mod cpp;
pub mod java;
pub mod rust;
pub mod go;
pub mod python;

// Re-exports of the plugin entry-point structs.
pub use c::plugin::CPlugin;
pub use rust::RustPlugin;
pub use go::GoPlugin;
pub use python::plugin::PythonPlugin;
