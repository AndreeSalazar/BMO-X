//! Plugin traits for language adapters.
//!
//! v2.0.0: single, clean LanguageAdapter trait. No more duplicate
//! `LanguagePlugin` vs `LanguageAdapter`, no more duplicate
//! `MemoryModel` vs `MemoryModel2`. One trait, one purpose:
//! compile a language's source to BMO AST or directly to native x86-64.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

/// Language identifier.
///
/// The BMO ABI is the filter: every language adapter produces calls
/// to the same BMO ABI syscalls (0x100..0x1FF). The Language enum is
/// just for routing and metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// BMO — the native first-class language.
    Bmo,
    /// C — the lingua franca of system programming.
    C,
    /// C++ — extension of C with classes.
    Cpp,
    /// Python — high-level scripting.
    Python,
    /// Java — JVM-style bytecode.
    Java,
    /// Custom language (id from registry).
    Custom(u32),
}

impl Language {
    pub fn name(&self) -> &'static str {
        match self {
            Language::Bmo     => "bmo",
            Language::C       => "c",
            Language::Cpp     => "cpp",
            Language::Python  => "python",
            Language::Java    => "java",
            Language::Custom(_) => "custom",
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            Language::Bmo     => "bmo",
            Language::C       => "c",
            Language::Cpp     => "cpp",
            Language::Python  => "py",
            Language::Java    => "java",
            Language::Custom(_) => "txt",
        }
    }
}

// ─── Error types ─────────────────────────────────────────────────────

/// Error from LanguageAdapter compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterError {
    SyntaxError,
    TypeError,
    ImportError,
    InternalError,
    NotSupported,
    AbiMismatch,  // language generates calls outside the BMO ABI
}

impl AdapterError {
    pub fn message(&self) -> &'static str {
        match self {
            AdapterError::SyntaxError  => "syntax error",
            AdapterError::TypeError    => "type error",
            AdapterError::ImportError  => "import error",
            AdapterError::InternalError => "internal compiler error",
            AdapterError::NotSupported => "not supported",
            AdapterError::AbiMismatch  => "ABI mismatch (use BMO ABI 0x100..0x1FF)",
        }
    }
}

/// Memory model of a language. Tells the BMO runtime how to handle
/// references from this language's code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryModel {
    Manual,            // C, C++: explicit alloc/free
    ReferenceCounted,  // Python, Swift: ARC
    GarbageCollected,  // Java, Go: GC
    Ownership,         // Rust: borrow checker
    Hybrid,            // Mix
}

/// GC strategy a language requires. `None` for manual or ownership
/// languages. Used by the runtime to wire up the right GC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcStrategy {
    None,
    ReferenceCounting,
    MarkSweep,
    Generational,
    Copying,
    Concurrent,
    Region,
}

// ─── The single trait ───────────────────────────────────────────────

/// Language adapter — the ONE trait a language must implement.
///
/// Implementors produce native x86-64 code that calls the BMO ABI
/// (syscalls 0x100..0x1FF). This is the filter: the adapter must
/// translate its language's idioms into BMO ABI calls.
pub trait LanguageAdapter: Send + Sync {
    /// Get the language this adapter handles.
    fn language(&self) -> Language;

    /// Get plugin name (matches Language::name()).
    fn name(&self) -> &'static str { self.language().name() }

    /// Get plugin version.
    fn version(&self) -> &'static str { "1.0.0" }

    /// File extensions this adapter handles.
    fn extensions(&self) -> &[&'static str];

    /// Compile source to native x86-64 machine code.
    ///
    /// The output must be a complete x86-64 function with entry
    /// point at offset 0. All kernel calls must go through the
    /// BMO ABI (see `super::super::abi` for the syscall table).
    fn compile_native(&self, source: &[u8]) -> Result<Vec<u8>, AdapterError>;

    /// Auto-detect if this adapter can compile the given source.
    fn can_compile(&self, source: &[u8]) -> bool;

    /// Light validation (just enough to reject obvious errors).
    fn validate(&self, _source: &[u8]) -> bool { true }

    /// Memory model of this language. Default: manual.
    fn memory_model(&self) -> MemoryModel { MemoryModel::Manual }

    /// GC strategy required. Default: none.
    fn gc_strategy(&self) -> GcStrategy { GcStrategy::None }

    /// Enable this plugin.
    fn enable(&mut self) {}

    /// Disable this plugin.
    fn disable(&mut self) {}
}
