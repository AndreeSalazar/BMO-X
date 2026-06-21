//! Plugin registry for managing language plugins.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use super::traits::{
    Language, LanguagePlugin, RuntimeConfig, LanguageFeatures, CompileResult,
    LanguageAdapter,
};

/// Plugin registry - manages all registered language plugins and adapters
pub struct LanguageRegistry {
    plugins: Vec<Box<dyn LanguagePlugin>>,
    adapters: Vec<Box<dyn LanguageAdapter>>,
    active_language: Option<Language>,
}

impl LanguageRegistry {
    /// Create empty registry
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            adapters: Vec::new(),
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
            None => Err(crate::bmo_gpu::BxError::Unsupported),
        }
    }

    /// Compile with active language
    pub fn compile_active(&self, source: &[u8]) -> BxResult<CompileResult> {
        match self.active_language {
            Some(lang) => self.compile(source, lang),
            None => Err(crate::bmo_gpu::BxError::Unsupported),
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

    /// Enable a plugin by name. v1.8.0: enables a previously-registered
    /// but disabled plugin.
    pub fn enable(&mut self, name: &str) -> bool {
        for plugin in &mut self.plugins {
            if plugin.name() == name {
                plugin.enable();
                return true;
            }
        }
        false
    }

    /// Disable a plugin by name.
    pub fn disable(&mut self, name: &str) -> bool {
        for plugin in &mut self.plugins {
            if plugin.name() == name {
                plugin.disable();
                return true;
            }
        }
        false
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
            None => Err(crate::bmo_gpu::BxError::Unsupported),
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

    // ── LanguageAdapter methods (compile to native x86-64) ──────────

    /// Register a language adapter (native compiler, no VM).
    pub fn register_adapter(&mut self, adapter: Box<dyn LanguageAdapter>) {
        self.adapters.push(adapter);
    }

    /// Find adapter by name.
    pub fn get_adapter(&self, name: &str) -> Option<&dyn LanguageAdapter> {
        self.adapters.iter().find(|a| a.name() == name).map(|a| a.as_ref())
    }

    /// Find adapter by file extension.
    pub fn get_adapter_by_extension(&self, ext: &str) -> Option<&dyn LanguageAdapter> {
        self.adapters.iter().find(|a| a.extensions().iter().any(|e| *e == ext)).map(|a| a.as_ref())
    }

    /// Auto-detect adapter from source content.
    pub fn detect_adapter(&self, source: &[u8]) -> Option<&dyn LanguageAdapter> {
        self.adapters.iter().find(|a| a.can_compile(source)).map(|a| a.as_ref())
    }

    /// Compile source to native x86-64 using auto-detected adapter.
    pub fn compile_native(&self, source: &[u8]) -> Result<Vec<u8>, super::traits::AdapterError> {
        if let Some(adapter) = self.detect_adapter(source) {
            return adapter.compile_native(source);
        }
        Err(super::traits::AdapterError::NotSupported)
    }

    /// Compile using a specific named adapter.
    pub fn compile_native_as(&self, name: &str, source: &[u8]) -> Result<Vec<u8>, super::traits::AdapterError> {
        if let Some(adapter) = self.get_adapter(name) {
            return adapter.compile_native(source);
        }
        Err(super::traits::AdapterError::NotSupported)
    }

    /// Number of registered adapters.
    pub fn adapter_count(&self) -> usize { self.adapters.len() }

    /// List all adapter names.
    pub fn adapter_names(&self) -> Vec<&str> {
        self.adapters.iter().map(|a| a.name()).collect()
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
