//! Python language plugin for ÑEXO.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::ToString;

use crate::bmo_gpu::BxResult;
use super::super::super::traits::{
    Language, LanguagePlugin, RuntimeConfig, LanguageFeatures, CompileResult, CompileError,
};
use super::lexer::PyLexer;
use super::parser::PyParser;
use super::translator::PyToNexo;

/// Python language plugin
pub struct PythonPlugin {
    version: &'static str,
}

impl PythonPlugin {
    pub fn new() -> Self {
        Self {
            version: "Python 3.11",
        }
    }
}

impl LanguagePlugin for PythonPlugin {
    fn language(&self) -> Language { Language::Python }

    fn name(&self) -> &'static str { "python" }

    fn runtime_config(&self) -> RuntimeConfig { RuntimeConfig::for_python() }

    fn compile(&self, source: &[u8]) -> BxResult<CompileResult> {
        // v0.1.0: lexer + parser + translator chain.
        let mut lex = PyLexer::new(source);
        let tokens = match lex.tokenize() {
            Ok(t) => t,
            Err(_) => return Ok(CompileResult {
                success: false,
                errors: alloc::vec![CompileError { message: "lex error".to_string(), line: 0, column: 0 }],
                warnings: Vec::new(),
                generated_code: None,
            }),
        };
        let mut parser = PyParser::new(tokens);
        let past = match parser.parse() {
            Ok(p) => p,
            Err(_) => return Ok(CompileResult {
                success: false,
                errors: alloc::vec![CompileError { message: "parse error".to_string(), line: 0, column: 0 }],
                warnings: Vec::new(),
                generated_code: None,
            }),
        };
        let translator = PyToNexo::new();
        let _ = translator.translate(&past);
        Ok(CompileResult {
            success: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            generated_code: None,
        })
    }

    fn features(&self) -> LanguageFeatures {
        LanguageFeatures {
            has_pointers: false,
            has_generics: true,  // Type hints
            has_traits: false,
            has_modules: true,
            has_macros: false,
            has_attributes: true,  // Decorators
            has_pattern_matching: true,  // match statement
            has_closures: true,
            has_async: true,  // async/await
            has_errors: true,  // try/except
            has_option: false,
            has_arrays: true,  // Lists
            has_slices: true,
            has_strings: true,
            has_maps: true,  // Dicts
            has_sets: true,
        }
    }

    fn validate(&self, source: &[u8]) -> BxResult<bool> {
        Ok(!source.is_empty())
    }
    fn version(&self) -> &'static str { self.version }
    fn description(&self) -> &'static str { "Python programming language plugin for ÑEXO" }

    fn can_compile(&self, source: &[u8]) -> bool {
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("def main") || text.contains("import ") || text.contains("class ")
    }
}
