//! Plugin registry for managing language plugins.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::barex::BxResult;
use super::traits::{
    Language, LanguagePlugin, RuntimeConfig, LanguageFeatures, CompileResult,
};

/// Plugin registry - manages all registered language plugins
pub struct LanguageRegistry {
    plugins: Vec<Box<dyn LanguagePlugin>>,
    active_language: Option<Language>,
}

impl LanguageRegistry {
    /// Create empty registry
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            active_language: None,
        }
    }

    /// Register a language plugin
    pub fn register(&mut self, plugin: Box<dyn LanguagePlugin>) {
        let lang = plugin.language();
        self.plugins.push(plugin);
        // Set first registered language as active
        if self.active_language.is_none() {
            self.active_language = Some(lang);
        }
    }

    /// Get plugin for language
    pub fn get(&self, lang: Language) -> Option<&dyn LanguagePlugin> {
        self.plugins.iter().find(|p| p.language() == lang).map(|p| p.as_ref())
    }

    /// Get mutable plugin for language
    pub fn get_mut(&mut self, lang: Language) -> Option<&mut Box<dyn LanguagePlugin>> {
        self.plugins.iter_mut().find(|p| p.language() == lang)
    }

    /// Compile source with appropriate plugin
    pub fn compile(&self, source: &[u8], lang: Language) -> BxResult<CompileResult> {
        match self.get(lang) {
            Some(plugin) => plugin.compile(source),
            None => Err(crate::barex::BxError::Unsupported),
        }
    }

    /// Compile with active language
    pub fn compile_active(&self, source: &[u8]) -> BxResult<CompileResult> {
        match self.active_language {
            Some(lang) => self.compile(source, lang),
            None => Err(crate::barex::BxError::Unsupported),
        }
    }

    /// Set active language
    pub fn set_active(&mut self, lang: Language) -> bool {
        if self.plugins.iter().any(|p| p.language() == lang) {
            self.active_language = Some(lang);
            true
        } else {
            false
        }
    }

    /// Get active language
    pub fn active_language(&self) -> Option<Language> {
        self.active_language
    }

    /// List all registered languages
    pub fn languages(&self) -> Vec<Language> {
        self.plugins.iter().map(|p| p.language()).collect()
    }

    /// Get count of registered plugins
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    /// Check if language is registered
    pub fn has_language(&self, lang: Language) -> bool {
        self.plugins.iter().any(|p| p.language() == lang)
    }

    /// Validate source with appropriate plugin
    pub fn validate(&self, source: &[u8], lang: Language) -> BxResult<bool> {
        match self.get(lang) {
            Some(plugin) => plugin.validate(source),
            None => Err(crate::barex::BxError::Unsupported),
        }
    }

    /// Get runtime config for language
    pub fn runtime_config(&self, lang: Language) -> Option<RuntimeConfig> {
        self.get(lang).map(|p| p.runtime_config())
    }

    /// Get features for language
    pub fn features(&self, lang: Language) -> Option<LanguageFeatures> {
        self.get(lang).map(|p| p.features())
    }

    /// Auto-detect language from source
    pub fn detect_language(&self, source: &[u8]) -> Option<Language> {
        for plugin in &self.plugins {
            if plugin.can_compile(source) {
                return Some(plugin.language());
            }
        }
        None
    }

    /// Get plugin by file extension
    pub fn get_by_extension(&self, ext: &str) -> Option<&dyn LanguagePlugin> {
        self.plugins.iter().find(|p| p.language().file_extension() == ext).map(|p| p.as_ref())
    }

    /// Get all supported extensions
    pub fn supported_extensions(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|p| p.language().file_extension()).collect()
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
