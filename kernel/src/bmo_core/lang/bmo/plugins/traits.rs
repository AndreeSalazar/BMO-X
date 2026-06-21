//! Core plugin traits for the BMO language system.
//!
//! These traits define the interface that all language plugins must implement.
//! Each language provides: Lexer → Parser → AST → Translator → BMO AST

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;

/// Language identifier.
///
/// v1.8.0: only the languages with actual plugin implementations are
/// listed. `Rust` and `Go` were removed — BMO is the native language
/// (no need for a Rust→BMO shim). Other languages (Swift, JS) are
/// planned for v2.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// C — the lingua franca of system programming.
    C,
    /// C++ — extension of C with classes.
    Cpp,
    /// Python — high-level scripting.
    Python,
    /// Java — JVM-like language.
    Java,
    /// Custom language (id from registry).
    Custom(u32),
}

impl Language {
    pub fn name(&self) -> &'static str {
        match self {
            Language::C => "C",
            Language::Cpp => "C++",
            Language::Python => "Python",
            Language::Java => "Java",
            Language::Custom(_) => "Custom",
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::Python => "py",
            Language::Java => "java",
            Language::Custom(_) => "txt",
        }
    }

    pub fn is_system_language(&self) -> bool {
        matches!(self, Language::C | Language::Cpp)
    }

    pub fn is_scripting_language(&self) -> bool {
        matches!(self, Language::Python)
    }
}

/// Memory management strategy for a language
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryModel {
    Manual,              // C, Rust (manual alloc/free)
    ReferenceCounted,    // Swift, Python (ARC/RC)
    GarbageCollected,    // Go, Java, Python (GC)
    Ownership,           // Rust (borrow checker)
    Hybrid,              // Combination (e.g., RC + cycle detection)
}

/// Garbage collection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcType {
    None,
    MarkSweep,
    Copying,
    Generational,
    ReferenceCounting,
    Concurrent,
    Incremental,
    RegionBased,
}

/// Global Interpreter Lock type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GilType {
    None,
    Traditional,         // Python-style GIL
    FineGrained,         // Per-object locks
    ReadWriteLock,       // Multiple readers, single writer
    LockFree,            // Lock-free data structures
}

/// Runtime configuration for a language
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub memory_model: MemoryModel,
    pub gc_type: GcType,
    pub gil_type: GilType,
    pub has_threads: bool,
    pub has_coroutines: bool,
    pub stack_size: usize,
    pub heap_size: usize,
    pub ffi_support: bool,
    pub max_call_depth: usize,
}

impl RuntimeConfig {
    pub fn for_c() -> Self {
        Self {
            memory_model: MemoryModel::Manual,
            gc_type: GcType::None,
            gil_type: GilType::None,
            has_threads: true,
            has_coroutines: false,
            stack_size: 8 * 1024,
            heap_size: 1024 * 1024,
            ffi_support: true,
            max_call_depth: 1024,
        }
    }

    pub fn for_rust() -> Self {
        Self {
            memory_model: MemoryModel::Ownership,
            gc_type: GcType::None,
            gil_type: GilType::None,
            has_threads: true,
            has_coroutines: true,
            stack_size: 8 * 1024,
            heap_size: 1024 * 1024,
            ffi_support: true,
            max_call_depth: 1024,
        }
    }

    pub fn for_go() -> Self {
        Self {
            memory_model: MemoryModel::GarbageCollected,
            gc_type: GcType::Concurrent,
            gil_type: GilType::None,
            has_threads: true,
            has_coroutines: true,
            stack_size: 1024,
            heap_size: 4 * 1024 * 1024,
            ffi_support: true,
            max_call_depth: 4096,
        }
    }

    pub fn for_python() -> Self {
        Self {
            memory_model: MemoryModel::ReferenceCounted,
            gc_type: GcType::ReferenceCounting,
            gil_type: GilType::Traditional,
            has_threads: false,
            has_coroutines: true,
            stack_size: 4 * 1024,
            heap_size: 2 * 1024 * 1024,
            ffi_support: true,
            max_call_depth: 512,
        }
    }

    pub fn for_java() -> Self {
        Self {
            memory_model: MemoryModel::GarbageCollected,
            gc_type: GcType::Generational,
            gil_type: GilType::None,
            has_threads: true,
            has_coroutines: false,
            stack_size: 512 * 1024,
            heap_size: 256 * 1024 * 1024,
            ffi_support: true,
            max_call_depth: 2048,
        }
    }
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
    pub has_errors: bool,
    pub has_option: bool,
    pub has_arrays: bool,
    pub has_slices: bool,
    pub has_strings: bool,
    pub has_maps: bool,
    pub has_sets: bool,
}

impl LanguageFeatures {
    pub fn minimal() -> Self {
        Self {
            has_pointers: false,
            has_generics: false,
            has_traits: false,
            has_modules: false,
            has_macros: false,
            has_attributes: false,
            has_pattern_matching: false,
            has_closures: false,
            has_async: false,
            has_errors: false,
            has_option: false,
            has_arrays: false,
            has_slices: false,
            has_strings: false,
            has_maps: false,
            has_sets: false,
        }
    }

    pub fn full() -> Self {
        Self {
            has_pointers: true,
            has_generics: true,
            has_traits: true,
            has_modules: true,
            has_macros: true,
            has_attributes: true,
            has_pattern_matching: true,
            has_closures: true,
            has_async: true,
            has_errors: true,
            has_option: true,
            has_arrays: true,
            has_slices: true,
            has_strings: true,
            has_maps: true,
            has_sets: true,
        }
    }
}

/// Result of compilation
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub success: bool,
    pub errors: Vec<CompileError>,
    pub warnings: Vec<CompileWarning>,
    pub generated_code: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct CompileWarning {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

/// Core language plugin trait
///
/// Each language implements this to compile through BMO.
pub trait LanguagePlugin: Send + Sync {
    /// Get language info
    fn language(&self) -> Language;

    /// Get plugin name (e.g. "c", "cpp", "python", "java")
    fn name(&self) -> &'static str;

    /// Get runtime configuration
    fn runtime_config(&self) -> RuntimeConfig;

    /// Compile source to BMO bytecode
    fn compile(&self, source: &[u8]) -> BxResult<CompileResult>;

    /// Enable this plugin (called by `registry.enable("name")`)
    fn enable(&mut self) {}

    /// Disable this plugin (called by `registry.disable("name")`)
    fn disable(&mut self) {}

    /// Get supported features
    fn features(&self) -> LanguageFeatures;

    /// Validate source code
    fn validate(&self, source: &[u8]) -> BxResult<bool>;

    /// Get language version
    fn version(&self) -> &'static str;

    /// Get language description
    fn description(&self) -> &'static str;

    /// Check if source is valid for this language
    fn can_compile(&self, source: &[u8]) -> bool;
}

/// Garbage collector plugin trait
pub trait GcPlugin: Send + Sync {
    /// Get GC type
    fn gc_type(&self) -> GcType;

    /// Initialize GC
    fn init(&mut self, heap_size: usize) -> BxResult<()>;

    /// Allocate memory
    fn alloc(&mut self, size: usize) -> BxResult<*mut u8>;

    /// Mark object as reachable
    fn mark(&mut self, ptr: *mut u8) -> BxResult<()>;

    /// Sweep unreachable objects
    fn sweep(&mut self) -> BxResult<usize>;

    /// Get GC statistics
    fn stats(&self) -> GcStats;

    /// Check if GC is needed
    fn needs_gc(&self) -> bool;

    /// Run GC cycle
    fn collect(&mut self) -> BxResult<usize>;
}

/// GC statistics
#[derive(Debug, Clone)]
pub struct GcStats {
    pub total_allocated: usize,
    pub total_freed: usize,
    pub live_objects: usize,
    pub collections: usize,
    pub pause_time_us: u64,
}

/// Global Interpreter Lock plugin trait
pub trait GilPlugin: Send + Sync {
    /// Get GIL type
    fn gil_type(&self) -> GilType;

    /// Acquire GIL
    fn acquire(&self) -> BxResult<()>;

    /// Release GIL
    fn release(&self) -> BxResult<()>;

    /// Check if GIL is held
    fn is_held(&self) -> bool;

    /// Try to acquire GIL (non-blocking)
    fn try_acquire(&self) -> bool;

    /// Get GIL statistics
    fn stats(&self) -> GilStats;
}

/// GIL statistics
#[derive(Debug, Clone)]
pub struct GilStats {
    pub acquisitions: u64,
    pub releases: u64,
    pub contention: u64,
    pub wait_time_us: u64,
}

/// ABI bridge trait for FFI
pub trait AbiBridge: Send + Sync {
    /// Get ABI name
    fn name(&self) -> &'static str;

    /// Initialize bridge
    fn init(&mut self) -> BxResult<()>;

    /// Call foreign function
    fn call(&self, name: &str, args: &[u8]) -> BxResult<Vec<u8>>;

    /// Register native function
    fn register(&mut self, name: &str, func: extern "C" fn()) -> BxResult<()>;

    /// Check if function exists
    fn has_function(&self, name: &str) -> bool;

    /// Get function signature
    fn get_signature(&self, name: &str) -> Option<AbiSignature>;
}

/// ABI function signature
#[derive(Debug, Clone)]
pub struct AbiSignature {
    pub name: String,
    pub params: Vec<AbiParam>,
    pub return_type: AbiType,
}

#[derive(Debug, Clone)]
pub struct AbiParam {
    pub name: String,
    pub ty: AbiType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiType {
    Void,
    Bool,
    I8, I16, I32, I64,
    U8, U16, U32, U64,
    F32, F64,
    Pointer,
    Struct(u32),
    Array(u32, u32),
}
