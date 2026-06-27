//! `lang::pipeline` — Orquesta el flujo frontend → backend → runtime.
//!
//! v1.8.8: implementación básica que toma source + language → BEF.
//!
//! ## Pipeline
//!
//! ```text
//! source bytes
//!   │
//!   ▼
//! frontend.compile_to_ir(source, name)
//!   │  (devuelve common::ast::Module)
//!   ▼
//! backend.compile_module(module)
//!   │  (devuelve bytes x86-64)
//!   ▼
//! runtime.c_min (linker decide si lo incluye)
//!   │
//!   ▼
//! BEF (BMO Executable Format)
//! ```

#![allow(dead_code)]

extern crate alloc;

use crate::bmo_gpu::BxResult;
use crate::lang::common::ast::Module;
use crate::lang::backends::aot_x86_64;

/// Idioma del source code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceLang {
    Bmo,
    C,
}

/// Resultado de la compilación completa.
pub struct CompiledProgram {
    /// Bytes x86-64 de la función principal.
    pub code: alloc::vec::Vec<u8>,
    /// Tamaño del runtime que necesita.
    pub runtime_size: u32,
    /// Lenguaje origen.
    pub lang: SourceLang,
}

/// Compila source code a un programa listo para ejecutar.
pub fn compile(source: &[u8], lang: SourceLang, name: &str) -> BxResult<CompiledProgram> {
    // 1. Frontend → BMO IR
    let module: Module = match lang {
        SourceLang::Bmo => crate::lang::frontends::bmo_frontend::compile_to_ir(source, name)?,
        SourceLang::C   => crate::lang::frontends::c::compile_to_ir(source, name)?,
    };

    // 2. Backend → BmoObject
    let obj = aot_x86_64::compile_module(&module)?;
    // 3. Linker v2.0 → BEF
    let bef = crate::lang::linker::link(obj)?;
    let code = bef.bytes;
    let runtime_size = bef.runtime_size;

    Ok(CompiledProgram { code, runtime_size, lang })
}
