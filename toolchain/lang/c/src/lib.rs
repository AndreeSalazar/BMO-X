//! **BMO C -- el FRONTEND**: preprocesador, lexer, parser, arbol, el juez de
//! tipos y la politica de libc.
//!
//! [isa] NINGUNA. No nombra una maquina y no depende de nada que emita: lo que
//! emite x86-64 vive en `emisor-x86_64/` (crate `bmo-c-x86-64`), igual que
//! INTI, Ada y COBOL. Partido el 2026-09-18 -- en BMO-X todo es x86-64 MENOS
//! los frontends, que son lo unico agnostico. Ver `toolchain/tools/isa`.
//!
//! [fase]     ARBOL
//!
//! [aparece]  AQUI -- la fachada de la crate
//!
//! [carril]   VERDE    -- si se rompe, ALGUIEN TE LO DICE antes de que salga de aqui
//!            * y sale de su `[aparece]`, no de una opinion: ver toolchain/tools/fases/

pub mod ast;
pub mod module;
pub mod parser;
pub mod standard;
mod lexer;
/// EL JUEZ UNICO de "que tipo es esta expresion". Ver su cabecera.
/// `pub` desde el 2026-09-18: el codegen, que lo consulta, vive en otro crate.
pub mod tipos;

use parser::Parser;

pub use standard::{CStandard, StandardFeatures};
#[cfg(test)]
use lexer::Token;

use std::path::{Path, PathBuf};
use ast::*;
pub fn parse(source: &str) -> Result<Program, CError> {
    let mut p = Parser::new(source);
    p.parse_program()
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

/// **Una unidad lista para emitir**: preprocesada, analizada y con la politica
/// de libc aplicada. La politica es de ENLACE, no de maquina, y por eso se
/// queda en el frontend (2026-09-18); el emisor solo le anade el `codegen`.
pub fn parse_unidad_con_preprocesador(
    source: &str,
    file_path: &Path,
    std: CStandard,
    libc: Libc,
) -> Result<Program, CError> {
    let features = StandardFeatures::load_standard(std);
    let include_paths = module::discover_include_paths();
    let mut pp = parser::preprocessor::Preprocessor::new(&features, include_paths);
    let expanded = pp.preprocess(source, file_path)?;
    let mut program = parse_with_features(&expanded, &features)?;
    politica_libc(&mut program, &pp.rangos_sistema, libc);
    Ok(program)
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


/// Aplica la politica a las funciones que vinieron de una cabecera del sistema.
///
/// * Se decide DESPUES de parsear y no dentro del parser a proposito: el parser
/// no tiene por que saber que existe un enlazador, y esto es exactamente una
/// decision de enlace. Lo unico que hace falta es el numero de linea, que el
/// arbol ya trae.
///
/// `pub` desde el 2026-09-18: C++ lee las cabeceras del sistema como C y les
/// aplica ESTA regla, no una copia de ella (ver `lang/cpp/src/preproceso.rs`).
pub fn politica_libc(program: &mut Program, rangos: &[(usize, usize)], libc: Libc) {
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
/// **El texto expandido Y que lineas vinieron de una cabecera del sistema**
/// (`<...>`), en rangos `(primera, ultima)` de base 1. Lo necesita C++, que lee
/// esas lineas como C (2026-09-18, `lang/cpp/src/preproceso.rs`). El mismo
/// preprocesador que `preprocess_only`: una sola forma de resolver un `#include`.
pub fn preprocesar_con_rangos(
    source: &str,
    file_path: &Path,
    std: CStandard,
) -> Result<(String, Vec<(usize, usize)>), CError> {
    let features = StandardFeatures::load_standard(std);
    let include_paths = module::discover_include_paths();
    let mut pp = parser::preprocessor::Preprocessor::new(&features, include_paths);
    let expandido = pp.preprocess(source, file_path)?;
    Ok((expandido, pp.rangos_sistema))
}

pub fn preprocess_only(source: &str, file_path: &Path, std: CStandard) -> Result<String, CError> {
    let features = StandardFeatures::load_standard(std);
    let include_paths = module::discover_include_paths();
    let mut pp = parser::preprocessor::Preprocessor::new(&features, include_paths);
    pp.preprocess(source, file_path)
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

/// **Analizar con modulos** (`use "..."`): resuelve los manifiestos y, si el
/// programa usa algo, le presta el catalogo de SYSCALL de la maquina.
///
/// ** `syscalls` lo pone quien emite (2026-09-18): el frontend no conoce los
/// numeros de la puerta de ninguna maquina.
pub fn parse_with_modules(
    source: &str,
    base_paths: Vec<PathBuf>,
    asm_paths: Option<Vec<PathBuf>>,
    syscalls: Vec<SyscallDef>,
) -> Result<Program, CError> {
    let mut resolver = module::ModuleResolver::new(base_paths).with_semantic_asm();
    let mut p = Parser::new(source);
    p.catalogo_syscalls = syscalls;
    p.parse_program_with_modules(&mut resolver, asm_paths)
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
mod tests {
    use super::*;

    /// La unica prueba del banco de C que mira DENTRO del lexer. Las demas
    /// compilan y ejecutan, y viven con el emisor (`emisor-x86_64/src/tests`).
    #[test]
    fn parses_use_directive() {
        let src = r#"use "bmo/core"; int main() { return 0; }"#;
        // tokenize and check
        let tokens = crate::Parser::tokenize_for_test(src);
        assert!(tokens.contains(&Token::Use), "should contain Use token");
    }
}
