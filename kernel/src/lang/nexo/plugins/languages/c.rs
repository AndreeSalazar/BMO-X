//! C language plugin for ÑEXO.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::barex::BxResult;
use super::super::traits::{
    Language, LanguagePlugin, RuntimeConfig, LanguageFeatures, CompileResult,
    CompileError, CompileWarning,
};

/// C language plugin
pub struct CPlugin {
    version: &'static str,
}

impl CPlugin {
    pub fn new() -> Self {
        Self {
            version: "C99/C11",
        }
    }
}

impl LanguagePlugin for CPlugin {
    fn language(&self) -> Language {
        Language::C
    }

    fn runtime_config(&self) -> RuntimeConfig {
        RuntimeConfig::for_c()
    }

    fn compile(&self, source: &[u8]) -> BxResult<CompileResult> {
        // Use existing C frontend
        let result = crate::lang::nexo::c::compile_c(source)?;

        Ok(CompileResult {
            success: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            generated_code: None,
        })
    }

    fn features(&self) -> LanguageFeatures {
        LanguageFeatures {
            has_pointers: true,
            has_generics: false,
            has_traits: false,
            has_modules: false,
            has_macros: true,
            has_attributes: false,
            has_pattern_matching: false,
            has_closures: false,
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
        // Basic validation
        if source.is_empty() {
            return Ok(false);
        }
        Ok(true)
    }

    fn version(&self) -> &'static str {
        self.version
    }

    fn description(&self) -> &'static str {
        "C programming language plugin for ÑEXO"
    }

    fn can_compile(&self, source: &[u8]) -> bool {
        // Check for C-like syntax
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("#include") || text.contains("int main") || text.contains("void ")
    }
}
