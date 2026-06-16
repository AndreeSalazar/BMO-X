//! Language plugin implementations for ÑEXO.
//!
//! Each language provides a complete plugin implementation:
//! - `c/` - C language plugin
//! - `rust/` - Rust language plugin
//! - `go/` - Go language plugin
//! - `python/` - Python language plugin

#![allow(dead_code)]

pub mod c;
pub mod rust;
pub mod go;
pub mod python;

// Re-exports
pub use c::CPlugin;
pub use rust::RustPlugin;
pub use go::GoPlugin;
pub use python::PythonPlugin;
