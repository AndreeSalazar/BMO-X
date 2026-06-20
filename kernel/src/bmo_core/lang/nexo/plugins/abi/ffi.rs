//! FFI (Foreign Function Interface) bridge implementation.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

use crate::bmo_core::barex::BxResult;

/// FFI function signature
#[derive(Debug, Clone)]
pub struct FfiSignature {
    pub name: String,
    pub params: Vec<FfiParam>,
    pub return_type: FfiType,
}

#[derive(Debug, Clone)]
pub struct FfiParam {
    pub name: String,
    pub ty: FfiType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiType {
    Void,
    Bool,
    I8, I16, I32, I64,
    U8, U16, U32, U64,
    F32, F64,
    Pointer,
    String,
}

/// FFI bridge for calling foreign functions
pub struct FfiBridge {
    functions: BTreeMap<String, FfiSignature>,
    initialized: bool,
}

impl FfiBridge {
    pub fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            initialized: false,
        }
    }

    /// Initialize the FFI bridge
    pub fn init(&mut self) -> BxResult<()> {
        self.initialized = true;
        Ok(())
    }

    /// Register a foreign function
    pub fn register_function(&mut self, sig: FfiSignature) {
        self.functions.insert(sig.name.clone(), sig);
    }

    /// Call a foreign function
    pub fn call(&self, name: &str, _args: &[u8]) -> BxResult<Vec<u8>> {
        if !self.functions.contains_key(name) {
            return Err(crate::bmo_core::barex::BxError::NotFound);
        }

        // Placeholder - would actually call the function
        Ok(Vec::new())
    }

    /// Check if function exists
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Get function signature
    pub fn get_signature(&self, name: &str) -> Option<&FfiSignature> {
        self.functions.get(name)
    }

    /// List all registered functions
    pub fn list_functions(&self) -> Vec<&str> {
        self.functions.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for FfiBridge {
    fn default() -> Self {
        Self::new()
    }
}
