//! ÑEXO Language Plugin System
//!
//! Allows multiple languages to compile through ÑEXO as the intermediate.
//! Each language provides: Lexer → Parser → AST → Translator → ÑEXO AST
//!
//! Supported languages:
//! - C (via c_frontend)
//! - Rust (future)
//! - Go (future)
//! - Python (future)

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::barex::BxResult;
use super::parser::{Ast, Stmt, Expr, TypeAnnotation};

/// Language identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    C,
    Rust,
    Go,
    Python,
    Java,
    Swift,
    Nex,
}

impl Language {
    pub fn name(&self) -> &'static str {
        match self {
            Language::C => "C",
            Language::Rust => "Rust",
            Language::Go => "Go",
            Language::Python => "Python",
            Language::Java => "Java",
            Language::Swift => "Swift",
            Language::Nex => "ÑEXO",
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            Language::C => "c",
            Language::Rust => "rs",
            Language::Go => "go",
            Language::Python => "py",
            Language::Java => "java",
            Language::Swift => "swift",
            Language::Nex => "nex",
        }
    }
}

/// Memory management strategy for a language
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryModel {
    Manual,         // C, Rust (manual alloc/free)
    ReferenceCounted, // Swift, Python (ARC/RC)
    GarbageCollected, // Go, Java, Python (GC)
    Ownership,       // Rust (borrow checker)
}

/// Runtime requirements for a language
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub memory_model: MemoryModel,
    pub has_gil: bool,                    // Global Interpreter Lock
    pub has_threads: bool,                // Native threading
    pub has_coroutines: bool,             // Async/await
    pub stack_size: usize,               // Default stack size
    pub heap_size: usize,                // Default heap size
    pub gc_type: GcType,                 // GC algorithm
    pub ffi_support: bool,               // Foreign Function Interface
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcType {
    None,
    MarkSweep,
    Copying,
    Generational,
    ReferenceCounting,
    Concurrent,
}

impl RuntimeConfig {
    pub fn for_c() -> Self {
        Self {
            memory_model: MemoryModel::Manual,
            has_gil: false,
            has_threads: true,
            has_coroutines: false,
            stack_size: 8 * 1024,         // 8 KB
            heap_size: 1024 * 1024,       // 1 MB
            gc_type: GcType::None,
            ffi_support: true,
        }
    }

    pub fn for_rust() -> Self {
        Self {
            memory_model: MemoryModel::Ownership,
            has_gil: false,
            has_threads: true,
            has_coroutines: true,
            stack_size: 8 * 1024,
            heap_size: 1024 * 1024,
            gc_type: GcType::None,
            ffi_support: true,
        }
    }

    pub fn for_go() -> Self {
        Self {
            memory_model: MemoryModel::GarbageCollected,
            has_gil: false,
            has_threads: true,             // goroutines
            has_coroutines: true,
            stack_size: 1024,              // 1 KB (grows)
            heap_size: 4 * 1024 * 1024,    // 4 MB
            gc_type: GcType::Concurrent,
            ffi_support: true,
        }
    }

    pub fn for_python() -> Self {
        Self {
            memory_model: MemoryModel::ReferenceCounted,
            has_gil: true,
            has_threads: false,            // True threads limited by GIL
            has_coroutines: true,          // async/await
            stack_size: 4 * 1024,          // 4 KB
            heap_size: 2 * 1024 * 1024,    // 2 MB
            gc_type: GcType::ReferenceCounting,
            ffi_support: true,
        }
    }
}

/// Language plugin trait
///
/// Each language implements this to compile through ÑEXO.
pub trait LanguagePlugin: Send + Sync {
    /// Get language info
    fn language(&self) -> Language;

    /// Get runtime configuration
    fn runtime_config(&self) -> RuntimeConfig;

    /// Compile source to ÑEXO AST
    fn compile(&self, source: &[u8]) -> BxResult<Ast>;

    /// Get supported features
    fn features(&self) -> LanguageFeatures;

    /// Validate source code
    fn validate(&self, source: &[u8]) -> BxResult<()>;
}

/// Features supported by a language
#[derive(Debug, Clone)]
pub struct LanguageFeatures {
    pub has_pointers: bool,
    pub has_generics: bool,
    pub has_traits: bool,
    pub has_modules: bool,
    pub has_macros: bool,
    pub has_attributes: bool,
    pub has_pattern_matching: bool,
    pub has_closures: bool,
    pub has_async: bool,
    pub has_errors: bool,         // Result/Either types
    pub has_option: bool,         // Option/Maybe types
}

/// Plugin registry
pub struct LanguageRegistry {
    plugins: Vec<Box<dyn LanguagePlugin>>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    /// Register a language plugin
    pub fn register(&mut self, plugin: Box<dyn LanguagePlugin>) {
        self.plugins.push(plugin);
    }

    /// Get plugin for language
    pub fn get(&self, lang: Language) -> Option<&dyn LanguagePlugin> {
        self.plugins.iter().find(|p| p.language() == lang).map(|p| p.as_ref())
    }

    /// Compile source with appropriate plugin
    pub fn compile(&self, source: &[u8], lang: Language) -> BxResult<Ast> {
        match self.get(lang) {
            Some(plugin) => plugin.compile(source),
            None => Err(crate::barex::BxError::Unsupported),
        }
    }

    /// List all registered languages
    pub fn languages(&self) -> Vec<Language> {
        self.plugins.iter().map(|p| p.language()).collect()
    }
}

/// Default registry with built-in languages
pub fn create_default_registry() -> LanguageRegistry {
    let mut registry = LanguageRegistry::new();

    // Register C plugin
    registry.register(Box::new(CPlugin));

    // Future: register other plugins
    // registry.register(Box::new(RustPlugin));
    // registry.register(Box::new(GoPlugin));
    // registry.register(Box::new(PythonPlugin));

    registry
}

/// C language plugin
struct CPlugin;

impl LanguagePlugin for CPlugin {
    fn language(&self) -> Language { Language::C }

    fn runtime_config(&self) -> RuntimeConfig { RuntimeConfig::for_c() }

    fn compile(&self, source: &[u8]) -> BxResult<Ast> {
        super::c::compile_c(source)
    }

    fn features(&self) -> LanguageFeatures {
        LanguageFeatures {
            has_pointers: true,
            has_generics: false,
            has_traits: false,
            has_modules: false,
            has_macros: true,        // #define
            has_attributes: false,
            has_pattern_matching: false,
            has_closures: false,
            has_async: false,
            has_errors: false,
            has_option: false,
        }
    }

    fn validate(&self, source: &[u8]) -> BxResult<()> {
        // Basic validation
        if source.is_empty() {
            return Err(crate::barex::BxError::InvalidArgument);
        }
        Ok(())
    }
}
