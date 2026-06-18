//! Java language plugin for ÑEXO.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::barex::BxResult;
use super::super::super::traits::{
    Language, LanguagePlugin, RuntimeConfig, LanguageFeatures, CompileResult, CompileError,
};
use super::lexer::JLexer;
use super::parser::JParser;
use super::translator::JavaToNexo;

pub struct JavaPlugin {
    version: &'static str,
}

impl JavaPlugin {
    pub fn new() -> Self { Self { version: "Java 17 (essential subset)" } }
}

impl LanguagePlugin for JavaPlugin {
    fn language(&self) -> Language { Language::Java }
    fn runtime_config(&self) -> RuntimeConfig { RuntimeConfig::for_python() /* share */ }

    fn compile(&self, source: &[u8]) -> BxResult<CompileResult> {
        // v0.1.0: lexer + parser + translator chain.
        let mut lex = JLexer::new(source);
        let tokens = match lex.tokenize() {
            Ok(t) => t,
            Err(_) => return Ok(CompileResult {
                success: false,
                errors: alloc::vec![CompileError { message: alloc::string::String::from("lex error"), line: 0, column: 0 }],
                warnings: Vec::new(),
                generated_code: None,
            }),
        };
        let mut parser = JParser::new(tokens);
        let jast = match parser.parse() {
            Ok(p) => p,
            Err(_) => return Ok(CompileResult {
                success: false,
                errors: alloc::vec![CompileError { message: alloc::string::String::from("parse error"), line: 0, column: 0 }],
                warnings: Vec::new(),
                generated_code: None,
            }),
        };
        let mut translator = JavaToNexo::new();
        let _ = translator.translate(&jast);
        Ok(CompileResult {
            success: true, errors: Vec::new(), warnings: Vec::new(), generated_code: None,
        })
    }

    fn features(&self) -> LanguageFeatures {
        LanguageFeatures {
            has_pointers: true,        // references
            has_generics: false,       // no type erasure
            has_traits: true,          // interfaces
            has_modules: false,        // no packages
            has_macros: false,         // no annotations runtime
            has_attributes: true,      // modifiers
            has_pattern_matching: false,
            has_closures: false,        // no lambdas
            has_async: false,
            has_errors: true,           // exceptions
            has_option: false,
            has_arrays: true,
            has_slices: false,
            has_strings: true,
            has_maps: false,            // no HashMap
            has_sets: false,
        }
    }

    fn validate(&self, source: &[u8]) -> BxResult<bool> { Ok(!source.is_empty()) }
    fn version(&self) -> &'static str { self.version }
    fn description(&self) -> &'static str {
        "Java essential subset (class, interface, virtual, try/catch) for ÑEXO"
    }

    fn can_compile(&self, source: &[u8]) -> bool {
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("class ") || text.contains("public class") || text.contains("interface ")
    }
}
