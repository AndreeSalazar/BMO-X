//! [fase]     ARBOL
//!
//! [aparece]  AQUI -- la fachada de la crate
//!
//! [carril]   VERDE    -- si se rompe, ALGUIEN TE LO DICE antes de que salga de aqui
//!            * y sale de su `[aparece]`, no de una opinion: ver toolchain/tools/fases/

pub mod codegen;
pub mod ast;
pub mod module;
pub mod parser;
pub mod standard;
mod lexer;
/// EL JUEZ UNICO de "que tipo es esta expresion". Ver su cabecera.
mod tipos;

use parser::Parser;

pub use standard::{CStandard, StandardFeatures};
#[cfg(test)]
use lexer::Token;

use std::path::{Path, PathBuf};
use ast::*;
use bmo_abi::profile::BmoLanguageProfile;

pub fn profile() -> BmoLanguageProfile {
    BmoLanguageProfile::C
}

pub fn parse(source: &str) -> Result<Program, CError> {
    let mut p = Parser::new(source);
    p.parse_program()
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

/// **Que hace la unidad con los cuerpos que traen las cabeceras del sistema.**
///
/// E2b y E5 de `docs/plan/PLAN_EL_ENLAZADOR.md`. Las cabeceras de BMO traen la
/// implementacion dentro --no habia enlazado, asi que no habia otro sitio-- y
/// eso, con varias unidades, es el mismo `strncpy` definido dos veces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Libc {
    /// **Cada unidad se queda su copia** (E2b). Es lo que ya pasaba con una
    /// sola unidad, y lo que hace `static inline` en una cabecera de C de
    /// verdad: el nombre no sale, asi que dos unidades no chocan.
    Copia,
    /// **La libc es de otro** (E5): los cuerpos no se emiten aqui, y sus
    /// nombres salen como indefinidos para que el enlazador los busque en
    /// `libc.bo`.
    Aparte,
    /// **Yo SOY la libc**: los cuerpos se emiten y sus nombres son publicos.
    /// Es como se construye `libc.bo`.
    Soy,
}

/// The same, through the preprocessor -- the path a real `.c` file takes.
pub fn compile_object_with_preprocessor(
    source: &str,
    file_path: &Path,
    std: CStandard,
    libc: Libc,
) -> Result<Vec<u8>, CError> {
    let features = StandardFeatures::load_standard(std);
    let include_paths = module::discover_include_paths();
    let mut pp = parser::preprocessor::Preprocessor::new(&features, include_paths);
    let expanded = pp.preprocess(source, file_path)?;
    let mut program = parse_with_features(&expanded, &features)?;
    politica_libc(&mut program, &pp.rangos_sistema, libc);
    codegen::compile_to_object(&program)
}

/// **El fuente de `libc.bo`**: una unidad que no hace mas que incluir las
/// cabeceras del sistema para que sus cuerpos se emitan UNA vez.
///
/// Se escribe aqui y no en un fichero del arbol porque no es codigo de nadie:
/// es la lista de cabeceras que TIENEN cuerpo. Las que solo traen constantes
/// (`limits.h`, `stdint.h`, `errno.h`) no aportan nada y no entran.
pub const FUENTE_LIBC: &str = "#include <stdio.h>\n\
                               #include <stdlib.h>\n\
                               #include <string.h>\n\
                               #include <strings.h>\n\
                               #include <ctype.h>\n\
                               #include <math.h>\n";

/// Compila `libc.bo`: los cuerpos de las cabeceras del sistema, una vez.
pub fn compile_libc_object(std: CStandard) -> Result<Vec<u8>, CError> {
    compile_object_with_preprocessor(FUENTE_LIBC, Path::new("libc.c"), std, Libc::Soy)
}

/// Aplica la politica a las funciones que vinieron de una cabecera del sistema.
///
/// * Se decide DESPUES de parsear y no dentro del parser a proposito: el parser
/// no tiene por que saber que existe un enlazador, y esto es exactamente una
/// decision de enlace. Lo unico que hace falta es el numero de linea, que el
/// arbol ya trae.
fn politica_libc(program: &mut Program, rangos: &[(usize, usize)], libc: Libc) {
    if libc == Libc::Soy || rangos.is_empty() {
        return;
    }
    let del_sistema = |linea: usize| rangos.iter().any(|(a, b)| linea >= *a && linea <= *b);
    match libc {
        Libc::Copia => {
            for f in &program.functions {
                if del_sistema(f.line) {
                    program.enlace.estaticos.insert(f.name.clone());
                }
            }
        }
        Libc::Aparte => {
            // Su firma se queda como PROTOTIPO --una llamada necesita los
            // tipos-- y el cuerpo se va: lo pone `libc.bo`.
            let (fuera, dentro): (Vec<_>, Vec<_>) =
                program.functions.drain(..).partition(|f| del_sistema(f.line));
            program.functions = dentro;
            for f in fuera {
                program.enlace.prototipos.push((
                    f.name.clone(),
                    f.params.iter().map(|p| p.typ.clone()).collect(),
                    f.ret_type.clone(),
                ));
            }
        }
        Libc::Soy => {}
    }
}

/// Compile with a specific C standard (C89/C99/C11/C17/C23).
/// Loads the standard TOML manifest and applies feature gating during parsing.
pub fn compile_with_standard(source: &str, std: CStandard) -> Result<Vec<u8>, CError> {
    let features = StandardFeatures::load_standard(std);
    let program = parse_with_features(source, &features)?;
    codegen::compile_to_bef_bytes(&program)
}

/// * Run ONLY the preprocessor and hand back the text it produced.
///
/// # Why this exists
///
/// Every error the compiler reports carries the line of the EXPANDED text, not
/// of the file the person wrote. With a couple of headers that is a small
/// annoyance; with DOOM, where `p_doors.c` expands past seven thousand lines,
/// it means the message names a line **nobody can look at** -- and then the
/// only way to find the construct is to guess it.
///
/// That happened, and guessing lost twice. So the fix is not a better message:
/// it is being able to open the line.
pub fn preprocess_only(source: &str, file_path: &Path, std: CStandard) -> Result<String, CError> {
    let features = StandardFeatures::load_standard(std);
    let include_paths = module::discover_include_paths();
    let mut pp = parser::preprocessor::Preprocessor::new(&features, include_paths);
    pp.preprocess(source, file_path)
}

/// Compile with full preprocessor pass (macros, includes, conditionals).
/// This is the recommended entry point for real C files.
pub fn compile_with_preprocessor(
    source: &str,
    file_path: &Path,
    std: CStandard,
) -> Result<Vec<u8>, CError> {
    let features = StandardFeatures::load_standard(std);

    // Run preprocessor: expand #include, #define, #ifdef, etc.
    let include_paths = module::discover_include_paths();
    let mut pp = parser::preprocessor::Preprocessor::new(&features, include_paths);
    let expanded = pp.preprocess(source, file_path)?;

    // Parse + compile the expanded source
    let program = parse_with_features(&expanded, &features)?;
    codegen::compile_to_bef_bytes(&program)
}

/// **Preprocesar y parsear, sin emitir.** Lo que necesita `--map`.
///
/// Existe porque `compile_with_preprocessor` hace las dos cosas y devuelve
/// bytes: para volcar el mapa de funciones hace falta el `Program`, y no hay
/// razon para escribir un `.bex` que nadie va a leer. Comparte cuerpo con la
/// otra --el preprocesador se instancia igual-- para que no haya dos formas de
/// resolver un `#include`.
pub fn parse_with_preprocessor(
    source: &str,
    file_path: &Path,
    std: CStandard,
) -> Result<Program, CError> {
    let features = StandardFeatures::load_standard(std);
    let include_paths = module::discover_include_paths();
    let mut pp = parser::preprocessor::Preprocessor::new(&features, include_paths);
    let expanded = pp.preprocess(source, file_path)?;
    parse_with_features(&expanded, &features)
}

/// Parse with standard feature gating.
pub fn parse_with_features(source: &str, features: &StandardFeatures) -> Result<Program, CError> {
    let mut p = Parser::new(source);
    p.features = features.clone();
    p.parse_program()
}

pub fn compile_source_to_bef_with_modules(source: &str, base_paths: Vec<PathBuf>) -> Result<Vec<u8>, CError> {
    let mut resolver = module::ModuleResolver::new(base_paths).with_semantic_asm();
    let program = Parser::new(source).parse_program_with_modules(&mut resolver, None)?;
    let used = module::find_used_functions(&program, &program.exported);
    codegen::compile_to_bef_bytes_filtered(&program, &used)
}

pub fn compile_source_to_bef_with_all(
    source: &str,
    base_paths: Vec<PathBuf>,
    asm_paths: Vec<PathBuf>,
) -> Result<Vec<u8>, CError> {
    let mut resolver = module::ModuleResolver::new(base_paths).with_semantic_asm();
    let program = Parser::new(source).parse_program_with_modules(&mut resolver, Some(asm_paths))?;
    let used = module::find_used_functions(&program, &program.exported);
    codegen::compile_to_bef_bytes_filtered(&program, &used)
}

#[derive(Debug, Clone)]
pub struct CError {
    pub line: usize,
    pub message: String,
}

impl CError {
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Self { line, message: message.into() }
    }
}




#[cfg(test)]
mod tests;
