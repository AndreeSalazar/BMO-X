//! BMO Plugin System (v2.0.0).
//!
//! Modular architecture for supporting multiple programming languages
//! through the BMO ABI as the integration point.
//!
//! # Architecture
//!
//! ```text
//!                   ┌─────────────┐
//!   C source  ─────▶│ C Adapter   │──┐
//!                   └─────────────┘  │
//!                   ┌─────────────┐  │     ┌──────────┐     ┌─────────────┐
//!   C++ source ────▶│ C++ Adapter │──┼────▶│ BMO ABI  │────▶│ BMO AOT     │──▶ x86-64
//!                   └─────────────┘  │     │ (filter) │     │ Compiler    │     native
//!                   ┌─────────────┐  │     │ 0x100..  │     │             │     code
//!   Java source ───▶│ Java Adapter│──┘     │ 0x1FF    │     └─────────────┘
//!                   └─────────────┘        └──────────┘
//!                   ┌─────────────┐
//!   BMO source  ───▶│ BMO Native  │─────────────────────────────────────▶
//!                   └─────────────┘
//! ```
//!
//! # Policy (v2.0.0)
//!
//! - **BMO is the only first-class high-level language**.
//! - **C, C++, Java, Python are optional plugins**. They are NOT
//!   loaded by default — the user activates them with
//!   `registry.enable("c")` etc.
//! - **The BMO ABI is the single filter**: every language adapter
//!   produces calls to the same BMO ABI syscalls (0x100..0x1FF). There
//!   is no language-specific ABI leakage.
//! - **No VM, no bytecode**: every language compiles to native x86-64
//!   via the BMO AOT compiler (or via a language-specific AOT that
//!   produces calls to the BMO ABI).
//!
//! # Quick start
//!
//! ```ignore
//! use crate::lang::bmo::plugins::registry::LanguageRegistry;
//!
//! let mut registry = LanguageRegistry::new();
//! // BMO is always available:
//! let bmo = registry.bmo_adapter();
//! // C is opt-in:
//! if registry.enable("c") {
//!     let c = registry.get("c").unwrap();
//!     c.compile_native(source)?;
//! }
//! ```

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;

pub mod abi_bridge;
pub mod languages;
pub mod registry;
pub mod traits;

pub use traits::{Language, LanguageAdapter, AdapterError, MemoryModel, GcStrategy};
pub use registry::LanguageRegistry;

/// Initialize the plugin system with all built-in language plugins.
///
/// BMO is always available. C is enabled by default for convenience
/// (the BMO kernel itself uses C-style headers internally). Other
/// languages are opt-in.
pub fn init_plugins() -> LanguageRegistry {
    let mut registry = LanguageRegistry::new();

    // BMO is always available.
    registry.register_bmo();

    // C is enabled by default (frontend C → BMO AST → AOT).
    // v1.8.8: C++/Java/Python se eliminaron. Si en el futuro se
    // añaden, crear `registry.register(Box::new(languages::XxxAdapter::new()));`
    // aquí.
    registry.register(Box::new(languages::CAdapter::new()));
    registry.enable("c");

    registry
}

/// List of languages the plugin system knows about.
pub fn supported_languages() -> alloc::vec::Vec<Language> {
    alloc::vec![
        Language::Bmo,
        Language::C,
        Language::Cpp,
        Language::Python,
        Language::Java,
    ]
}
