//! ÑEXO Language Plugin System
//!
//! Modular architecture for supporting multiple programming languages
//! through ÑEXO as the intermediate representation.
//!
//! Structure:
//! - `traits.rs` - Core plugin traits
//! - `registry.rs` - Plugin registry and management
//! - `languages/` - Language-specific implementations (C, Rust, Go, Python)
//! - `gc/` - Garbage collection strategies
//! - `gil/` - Global Interpreter Lock implementations
//! - `abi/` - Foreign Function Interface (FFI) bridge

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;

pub mod traits;
pub mod registry;
pub mod languages;
pub mod gc;
pub mod gil;
pub mod abi;

// Re-exports for convenience
pub use traits::{
    Language, AbiBridge,
};
pub use registry::LanguageRegistry;
pub use languages::{CPlugin, RustPlugin, GoPlugin, PythonPlugin};

/// Initialize the plugin system with all built-in plugins
pub fn init_plugins() -> LanguageRegistry {
    let mut registry = LanguageRegistry::new();

    // Register built-in language plugins
    registry.register(Box::new(CPlugin::new()));
    registry.register(Box::new(RustPlugin::new()));
    registry.register(Box::new(GoPlugin::new()));
    registry.register(Box::new(PythonPlugin::new()));

    registry
}

/// Get list of supported languages
pub fn supported_languages() -> Vec<Language> {
    alloc::vec![
        Language::C,
        Language::Rust,
        Language::Go,
        Language::Python,
    ]
}

/// Check if a language is supported
pub fn is_language_supported(lang: Language) -> bool {
    matches!(
        lang,
        Language::C | Language::Rust | Language::Go | Language::Python
    )
}
