//! Language plugin implementations for BMO (v1.8.0).
//!
//! Each language provides a complete plugin implementation:
//! - `c/`     - C language plugin (full frontend)
//! - `cpp/`   - C++ language plugin (essential subset)
//! - `python/`- Python language plugin
//! - `java/`  - Java language plugin
//!
//! Each language lives in its own subdirectory so it can have its own
//! lexer, parser, AST, translator, and tests. They all produce BMO
//! bytecode via `bmo::codegen`.
//!
//! v1.8.0: Rust and Go plugins are removed. BMO is the native language
//! (no need for Rust→BMO shim).

#![allow(dead_code)]

pub mod c;
pub mod cpp;
pub mod java;
pub mod python;

// Re-exports of the plugin entry-point structs.
pub use c::plugin::CPlugin;
pub use cpp::plugin::CppPlugin;
pub use python::plugin::PythonPlugin;
pub use java::plugin::JavaPlugin;
