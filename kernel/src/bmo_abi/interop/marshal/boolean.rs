//! Bool marshalling. BMO usa `bx_bool` (alias de `bool`), Win32 BOOL usa 4 bytes.

use crate::bmo_abi::primitives::{bx_bool, bx_i32};

#[inline(always)]
pub const fn bool_to_bmo(b: bool) -> bx_bool { b }

#[inline(always)]
pub const fn bmo_to_bool(b: bx_bool) -> bool { b }

/// Win32 `BOOL` (typedef `int`). Cualquier valor != 0 → true.
#[inline(always)]
pub const fn win32_bool_to_bmo(b: bx_i32) -> bx_bool { b != 0 }
