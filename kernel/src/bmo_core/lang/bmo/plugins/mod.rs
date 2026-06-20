//! BMO Plugin System (v1.8.0).
//!
//! Modular architecture for supporting multiple programming languages
//! through BMO as the intermediate representation.
//!
//! # Estructura
//!
//! ```text
//! bmo/plugins/
//! ├── traits.rs       — Plugin traits (Language, AbiBridge)
//! ├── registry.rs     — Plugin registry
//! ├── languages/      — C, C++, Java, Python (cualquiera puede ser un plugin)
//! ├── gc/             — Garbage collection strategies
//! ├── gil/            — Global Interpreter Lock implementations
//! └── abi/            — FFI bridge
//! ```
//!
//! # Política (v1.8.0)
//!
//! - **BMO es el único lenguaje de alto nivel nativo** (inspirado en CMD + Rust + ADA).
//! - **C, C++, Java, Python** son plugins opcionales. No se cargan por
//!   defecto — el usuario los activa con `bmo.plugins.enable("c")` etc.
//! - **El sistema de plugins es opt-in**. Si un plugin no está
//!   disponible, BMO no falla — sólo no soporta ese lenguaje.

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
pub use traits::{Language, AbiBridge};
pub use registry::LanguageRegistry;
pub use languages::{CPlugin, CppPlugin, JavaPlugin, PythonPlugin};

/// Initialize the plugin system with all built-in plugins.
///
/// v1.8.0: only the C plugin is enabled by default. Other plugins
/// are loaded on-demand via `registry.enable("c")` etc.
pub fn init_plugins() -> LanguageRegistry {
    let mut registry = LanguageRegistry::new();

    // Register all built-in language plugins (enabled lazily)
    registry.register(Box::new(CPlugin::new()));
    registry.register(Box::new(CppPlugin::new()));
    registry.register(Box::new(JavaPlugin::new()));
    registry.register(Box::new(PythonPlugin::new()));

    // C is enabled by default. Others are opt-in.
    registry.enable("c");

    registry
}

/// Get list of supported languages.
pub fn supported_languages() -> Vec<Language> {
    alloc::vec![
        Language::C,
        Language::Cpp,
        Language::Java,
        Language::Python,
    ]
}

/// Check if a language is supported.
pub fn is_language_supported(lang: Language) -> bool {
    matches!(
        lang,
        Language::C | Language::Cpp | Language::Java | Language::Python
    )
}
