//! Plugin registry for managing language adapters.
//!
//! v2.0.0: simplified. Only one trait (`LanguageAdapter`), no more
//! dual `LanguagePlugin`/`LanguageAdapter` paths. BMO is always
//! available; other languages are opt-in.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;

use super::traits::{Language, LanguageAdapter, AdapterError};
use super::languages::BmoAdapter;

/// Plugin registry — manages all registered language adapters.
pub struct LanguageRegistry {
    adapters: Vec<Box<dyn LanguageAdapter>>,
    active_language: Option<Language>,
}

impl LanguageRegistry {
    /// Create empty registry.
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
            active_language: None,
        }
    }

    /// Register a language adapter.
    pub fn register(&mut self, adapter: Box<dyn LanguageAdapter>) {
        let lang = adapter.language();
        self.adapters.push(adapter);
        if self.active_language.is_none() {
            self.active_language = Some(lang);
        }
    }

    /// Register the built-in BMO adapter. Always succeeds.
    pub fn register_bmo(&mut self) {
        self.register(Box::new(BmoAdapter::new()));
    }

    /// Get adapter for a language.
    pub fn get(&self, lang: Language) -> Option<&dyn LanguageAdapter> {
        self.adapters.iter().find(|a| a.language() == lang).map(|a| a.as_ref())
    }

    /// Get adapter by name.
    pub fn get_by_name(&self, name: &str) -> Option<&dyn LanguageAdapter> {
        self.adapters.iter().find(|a| a.name() == name).map(|a| a.as_ref())
    }

    /// Get adapter by file extension.
    pub fn get_by_extension(&self, ext: &str) -> Option<&dyn LanguageAdapter> {
        self.adapters.iter().find(|a| a.extensions().iter().any(|e| *e == ext)).map(|a| a.as_ref())
    }

    /// Auto-detect adapter from source content.
    pub fn detect(&self, source: &[u8]) -> Option<&dyn LanguageAdapter> {
        self.adapters.iter().find(|a| a.can_compile(source)).map(|a| a.as_ref())
    }

    /// Set the active language.
    pub fn set_active(&mut self, lang: Language) -> bool {
        if self.adapters.iter().any(|a| a.language() == lang) {
            self.active_language = Some(lang);
            true
        } else {
            false
        }
    }

    /// Enable a previously-registered plugin by name.
    pub fn enable(&mut self, name: &str) -> bool {
        for adapter in &mut self.adapters {
            if adapter.name() == name {
                adapter.enable();
                return true;
            }
        }
        false
    }

    /// Disable a plugin by name.
    pub fn disable(&mut self, name: &str) -> bool {
        for adapter in &mut self.adapters {
            if adapter.name() == name {
                adapter.disable();
                return true;
            }
        }
        false
    }

    /// Compile source with the auto-detected adapter.
    pub fn compile_native(&self, source: &[u8]) -> Result<Vec<u8>, AdapterError> {
        if let Some(adapter) = self.detect(source) {
            return adapter.compile_native(source);
        }
        // Fallback: use the active language.
        if let Some(lang) = self.active_language {
            if let Some(adapter) = self.get(lang) {
                return adapter.compile_native(source);
            }
        }
        Err(AdapterError::NotSupported)
    }

    /// Compile using a specific named adapter.
    pub fn compile_native_as(&self, name: &str, source: &[u8]) -> Result<Vec<u8>, AdapterError> {
        if let Some(adapter) = self.get_by_name(name) {
            return adapter.compile_native(source);
        }
        Err(AdapterError::NotSupported)
    }

    /// Get mutable reference to a plugin by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Box<dyn LanguageAdapter>> {
        self.adapters.iter_mut().find(|a| a.name() == name)
    }

    /// Get mutable reference to a plugin by language.
    pub fn get_lang_mut(&mut self, lang: Language) -> Option<&mut Box<dyn LanguageAdapter>> {
        self.adapters.iter_mut().find(|a| a.language() == lang)
    }

    /// Get the BMO adapter (always available).
    pub fn bmo_adapter(&self) -> &dyn LanguageAdapter {
        self.get(Language::Bmo).expect("BMO adapter is always present")
    }

    /// Get the active language.
    pub fn active_language(&self) -> Option<Language> { self.active_language }

    /// List all registered languages.
    pub fn languages(&self) -> Vec<Language> {
        self.adapters.iter().map(|a| a.language()).collect()
    }

    /// List all adapter names.
    pub fn names(&self) -> Vec<&'static str> {
        self.adapters.iter().map(|a| a.name()).collect()
    }

    /// Get count of registered adapters.
    pub fn count(&self) -> usize { self.adapters.len() }

    /// Check if a language is registered.
    pub fn has_language(&self, lang: Language) -> bool {
        self.adapters.iter().any(|a| a.language() == lang)
    }

    /// Check if a name is registered.
    pub fn has_name(&self, name: &str) -> bool {
        self.adapters.iter().any(|a| a.name() == name)
    }

    /// Light validation with the auto-detected adapter.
    pub fn validate(&self, source: &[u8]) -> bool {
        self.detect(source).map(|a| a.validate(source)).unwrap_or(false)
    }

    /// Light validation with a specific adapter by name.
    pub fn validate_as(&self, name: &str, source: &[u8]) -> bool {
        self.get_by_name(name).map(|a| a.validate(source)).unwrap_or(false)
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self { Self::new() }
}
