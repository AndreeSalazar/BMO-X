//! ABI bridge — FFI helpers for cross-ABI calls.
//!
//! v2.0.0: minimal. The BMO ABI IS the only ABI, so the bridge is
//! mostly empty. This module exists to leave room for future
//! ARM64 trampolines or language-specific boxing.

#![allow(dead_code)]

/// Initialize the ABI bridge. No-op in the current implementation.
pub fn init() {}

/// Validate a BMO ABI function name.
pub fn is_valid_abi_name(name: &str) -> bool {
    super::super::abi::is_abi(name)
}
