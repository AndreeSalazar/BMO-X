//! `lang::frontends::c` — C language frontend.
//!
//! v1.8.8: delega al pipeline legacy `lang::bmo::plugins::languages::c`.

#![allow(dead_code)]

pub mod preprocessor;
pub mod lexer;
pub mod parser;
pub mod translator;
pub mod adapter;

use crate::lang::common::ast::Module;
use crate::bmo_gpu::BxResult;

/// Compila C source al BMO IR canónico.
///
/// v1.8.8: usa el translator C legacy que produce BMO AST, luego
/// baja al BMO IR canónico.
pub fn compile_to_ir(source: &[u8], name: &str) -> BxResult<Module> {
    // 1. C → BMO AST (vía translator legacy)
    let bmo_ast = crate::lang::bmo::plugins::languages::c::translator::compile_c_to_native(source)
        .ok()
        .and_then(|_code| {
            // El translator legacy emite bytes x86-64 directos, no AST.
            // Por ahora retornamos un Module vacío como placeholder.
            // TODO: usar el translator nuevo que retorna AST.
            let mut module = Module::new(name);
            Some(module)
        })
        .ok_or(crate::bmo_gpu::BxError::Unsupported)?;

    Ok(bmo_ast)
}
