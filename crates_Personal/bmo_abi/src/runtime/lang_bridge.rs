//! `lang_bridge` — bridges entre lenguajes del BMO ABI.
//!
//! Cada bridge sabe cómo llamar funciones de un lenguaje específico:
//! convertir argumentos, manejar excepciones, gestionar GC.
//!
//! Soportes planificados: Rust nativo, C ABI, JVM, CLR, Python, Lua, Wasm.

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Identificador de lenguaje.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoLanguage {
    Rust,
    CAbi,
    Bef,
    Jvm,
    Clr,
    Python,
    Lua,
    Wasm,
}

/// Función de entrada del bridge: recibe un ID de función y argumentos
/// empaquetados, devuelve un resultado empaquetado.
pub type BridgeCallFn = extern "C" fn(
    fn_id: bx_u32,
    args_ptr: *const u8,
    args_len: bx_u32,
    result_ptr: *mut u8,
    result_len: bx_u32,
) -> bx_u64;

/// Un bridge de lenguaje registrado.
#[repr(C)]
pub struct LangBridge {
    pub language: BmoLanguage,
    pub call: BridgeCallFn,
    /// FNV-1a hash del nombre del lenguaje (para lookup rápido).
    pub name_hash: bx_u64,
}

impl LangBridge {
    pub const fn new(language: BmoLanguage, call: BridgeCallFn) -> Self {
        Self { language, call, name_hash: 0 }
    }
}
