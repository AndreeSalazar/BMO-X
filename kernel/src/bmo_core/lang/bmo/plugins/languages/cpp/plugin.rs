//! C++ language plugin for ÑEXO.
//!
//! v0.1.0: stub. The translator lowers C++ to ÑEXO AST, then the
//! standard codegen → BMO assembly syntax (legacy) → native pipeline runs.

#![allow(dead_code)]

use crate::bmo_gpu::BxResult;
use super::super::super::traits::{
    Language, LanguagePlugin, RuntimeConfig, LanguageFeatures, CompileResult,
};

pub struct CppPlugin {
    version: &'static str,
}

impl CppPlugin {
    pub fn new() -> Self {
        Self { version: "C++11 (essential subset)" }
    }
}

impl LanguagePlugin for CppPlugin {
    fn language(&self) -> Language { Language::Cpp }

    fn name(&self) -> &'static str { "cpp" }


    fn runtime_config(&self) -> RuntimeConfig { RuntimeConfig::for_c() }

    fn compile(&self, source: &[u8]) -> BxResult<CompileResult> {
        // TODO: full C++ → ÑEXO pipeline (lexer/parser/translator).
        // For now just acknowledge the source.
        let _ = source;
        Ok(CompileResult {
            success: true,
            errors: alloc::vec::Vec::new(),
            warnings: alloc::vec::Vec::new(),
            generated_code: None,
        })
    }

    fn features(&self) -> LanguageFeatures {
        LanguageFeatures {
            has_pointers: true,
            has_generics: false,        // templates not supported
            has_traits: false,
            has_modules: false,
            has_macros: true,
            has_attributes: true,       // __attribute__
            has_pattern_matching: false,
            has_closures: false,         // lambdas not supported
            has_async: false,
            has_errors: false,
            has_option: false,
            has_arrays: true,
            has_slices: false,
            has_strings: true,
            has_maps: false,
            has_sets: false,
        }
    }

    fn validate(&self, source: &[u8]) -> BxResult<bool> {
        Ok(!source.is_empty())
    }

    fn version(&self) -> &'static str { self.version }

    fn description(&self) -> &'static str {
        "C++ essential subset (class, virtual, new/delete) for ÑEXO"
    }

    fn can_compile(&self, source: &[u8]) -> bool {
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("class ") || text.contains("public:") || text.contains("virtual ")
    }
}
