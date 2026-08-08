pub mod codegen;
pub mod ast;
pub mod module;
pub mod ir_emit;
pub mod parser;
pub mod standard;
mod lexer;

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

/// Parse with standard feature gating.
pub fn parse_with_features(source: &str, features: &StandardFeatures) -> Result<Program, CError> {
    let mut p = Parser::new(source);
    p.features = features.clone();
    p.parse_program()
}

/// Compile C source to a unified IrModule (language-agnostic IR).
pub fn compile_to_ir(source: &str) -> Result<bmo_abi::ir::IrModule, CError> {
    let program = parse(source)?;
    Ok(ir_emit::compile_to_ir(&program))
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
