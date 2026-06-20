//! Rust language plugin for ÑEXO.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::bmo_core::barex::BxResult;
use super::super::traits::{
    Language, LanguagePlugin, RuntimeConfig, LanguageFeatures, CompileResult,
};

/// Rust language plugin
pub struct RustPlugin {
    version: &'static str,
}

impl RustPlugin {
    pub fn new() -> Self {
        Self {
            version: "Rust 2021",
        }
    }
}

impl LanguagePlugin for RustPlugin {
    fn language(&self) -> Language {
        Language::Rust
    }

    fn runtime_config(&self) -> RuntimeConfig {
        RuntimeConfig::for_rust()
    }

    fn compile(&self, _source: &[u8]) -> BxResult<CompileResult> {
        // Rust compilation would go here
        Ok(CompileResult {
            success: false,
            errors: Vec::new(),
            warnings: Vec::new(),
            generated_code: None,
        })
    }

    fn features(&self) -> LanguageFeatures {
        LanguageFeatures {
            has_pointers: true,
            has_generics: true,
            has_traits: true,
            has_modules: true,
            has_macros: true,
            has_attributes: true,
            has_pattern_matching: true,
            has_closures: true,
            has_async: true,
            has_errors: true,
            has_option: true,
            has_arrays: true,
            has_slices: true,
            has_strings: true,
            has_maps: true,
            has_sets: true,
        }
    }

    fn validate(&self, source: &[u8]) -> BxResult<bool> {
        if source.is_empty() {
            return Ok(false);
        }
        Ok(true)
    }

    fn version(&self) -> &'static str {
        self.version
    }

    fn description(&self) -> &'static str {
        "Rust programming language plugin for ÑEXO"
    }

    fn can_compile(&self, source: &[u8]) -> bool {
        let text = core::str::from_utf8(source).unwrap_or("");
        text.contains("fn main") || text.contains("let mut") || text.contains("impl ")
    }
}
