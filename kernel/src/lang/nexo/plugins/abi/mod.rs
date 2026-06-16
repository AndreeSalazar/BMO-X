//! ABI (Application Binary Interface) bridge module for ÑEXO.
//!
//! Provides FFI support for calling foreign functions and registering native functions.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

pub mod ffi;
pub mod types;

// Re-exports
pub use ffi::FfiBridge;
pub use types::{AbiType, AbiSignature, AbiParam};

use crate::barex::BxResult;
use super::traits::{AbiBridge, AbiType as TraitAbiType};

/// ABI bridge implementation
pub struct NexoAbiBridge {
    functions: Vec<(String, AbiSignature)>,
    initialized: bool,
}

impl NexoAbiBridge {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            initialized: false,
        }
    }
}

impl AbiBridge for NexoAbiBridge {
    fn name(&self) -> &'static str {
        "nexo-abi"
    }

    fn init(&mut self) -> BxResult<()> {
        self.initialized = true;
        Ok(())
    }

    fn call(&self, _name: &str, _args: &[u8]) -> BxResult<Vec<u8>> {
        // Placeholder - would call actual foreign function
        Ok(Vec::new())
    }

    fn register(&mut self, name: &str, _func: extern "C" fn()) -> BxResult<()> {
        let sig = AbiSignature {
            name: name.to_string(),
            params: Vec::new(),
            return_type: AbiType::Void,
        };
        self.functions.push((name.to_string(), sig));
        Ok(())
    }

    fn has_function(&self, name: &str) -> bool {
        self.functions.iter().any(|(n, _)| n == name)
    }

    fn get_signature(&self, name: &str) -> Option<AbiSignature> {
        self.functions.iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s.clone())
    }
}
