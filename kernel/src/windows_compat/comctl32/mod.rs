//! comctl32.dll compatibility — Common controls.

#![allow(dead_code)]

/// InitCommonControlsEx — initialize common controls.
#[no_mangle]
pub extern "C" fn InitCommonControlsEx(_icc: u64) -> u64 { 1 }
