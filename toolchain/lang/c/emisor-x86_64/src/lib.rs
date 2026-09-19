//! **BMO C para x86-64** -- del arbol de C a un `.bex` o a un `.bo`.
//!
//! [isa] x86-64 -- el UNICO sitio de C que nombra una maquina: el codegen, el
//! catalogo de syscalls de BMO-X y el perfil. El frontend (`bmo-c-front`, la
//! carpeta de arriba) analiza y no sabe de CPU. Partido el 2026-09-18
//! (`toolchain/tools/isa`).
//!
//! Todo lo del frontend se re-exporta tal cual (`pub use bmo_c_front::*`), asi
//! que `crate::ast`, `crate::tipos` o `crate::CError` siguen siendo los mismos
//! caminos para el codegen y el banco, y C++ y el enlazador solo cambian de
//! crate.
//!
//! [fase]     IMAGEN
//!
//! [aparece]  AQUI -- la fachada del emisor: devuelve el `.bex` o el `.bo`
//!
//! [carril]   VERDE    -- si se rompe, ALGUIEN TE LO DICE antes de que salga de aqui
//!            * y sale de su `[aparece]`, no de una opinion: ver toolchain/tools/fases/

pub use bmo_c_front::*;

pub mod codegen;

use std::path::{Path, PathBuf};
use ast::*;
use bmo_abi::profile::BmoLanguageProfile;

pub fn profile() -> BmoLanguageProfile {
    BmoLanguageProfile::C
}

/// El catalogo de SYSCALL de BMO-X: los numeros de ESTA maquina. El frontend
/// no los conoce; se los pasa esto cuando un programa dice `use`.
pub fn syscalls_de_bmo() -> Vec<SyscallDef> {
    bmo_abi::asm::defs::syscalls()
        .into_iter()
        .map(|d| SyscallDef { name: d.name, nr: d.nr, arg_count: d.arg_count })
        .collect()
}

pub fn compile_source_to_bef(source: &str) -> Result<Vec<u8>, CError> {
    let program = parse(source)?;
    codegen::compile_to_bef_bytes(&program)
}

/// **Compile ONE unit to an object (`.bo`)**, to be joined by `bmo-enlazar`.
/// E2 of `docs/plan/PLAN_EL_ENLAZADOR.md`; the contract is
/// `bmo_abi::bef::objeto`.
pub fn compile_source_to_object(source: &str) -> Result<Vec<u8>, CError> {
    let program = parse(source)?;
    codegen::compile_to_object(&program)
}

/// The same, through the preprocessor -- the path a real `.c` file takes.
pub fn compile_object_with_preprocessor(
    source: &str,
    file_path: &Path,
    std: CStandard,
    libc: Libc,
) -> Result<Vec<u8>, CError> {
    let program = parse_unidad_con_preprocesador(source, file_path, std, libc)?;
    codegen::compile_to_object(&program)
}

/// Compila `libc.bo`: los cuerpos de las cabeceras del sistema, una vez.
pub fn compile_libc_object(std: CStandard) -> Result<Vec<u8>, CError> {
    compile_object_with_preprocessor(FUENTE_LIBC, Path::new("libc.c"), std, Libc::Soy)
}

/// Compile with a specific C standard (C89/C99/C11/C17/C23).
/// Loads the standard TOML manifest and applies feature gating during parsing.
pub fn compile_with_standard(source: &str, std: CStandard) -> Result<Vec<u8>, CError> {
    let features = StandardFeatures::load_standard(std);
    let program = parse_with_features(source, &features)?;
    codegen::compile_to_bef_bytes(&program)
}

/// Compile with full preprocessor pass (macros, includes, conditionals).
/// This is the recommended entry point for real C files.
pub fn compile_with_preprocessor(
    source: &str,
    file_path: &Path,
    std: CStandard,
) -> Result<Vec<u8>, CError> {
    // El preprocesador y el analisis son del frontend (`parse_with_preprocessor`):
    // un solo sitio donde se resuelve un `#include`.
    let program = parse_with_preprocessor(source, file_path, std)?;
    codegen::compile_to_bef_bytes(&program)
}

pub fn compile_source_to_bef_with_modules(source: &str, base_paths: Vec<PathBuf>) -> Result<Vec<u8>, CError> {
    let program = parse_with_modules(source, base_paths, syscalls_de_bmo())?;
    let used = module::find_used_functions(&program, &program.exported);
    codegen::compile_to_bef_bytes_filtered(&program, &used)
}

#[cfg(test)]
mod tests;
