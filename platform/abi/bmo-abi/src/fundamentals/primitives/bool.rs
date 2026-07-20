//! `bx_bool` — bool canónico del BMO ABI. Reemplaza `<stdbool.h>`.
//!
//! 1 byte (vs el `BOOL` 32-bit de Win32 que desperdicia 24 bits). Same
//! representation que el `bool` de Rust → conversiones gratis.

#![allow(non_camel_case_types)]

pub type bx_bool = bool;

pub const BX_TRUE: bx_bool = true;
pub const BX_FALSE: bx_bool = false;

/// Helper: convierte un `bx_u32` (estilo Win32 `BOOL`) a `bx_bool`.
/// Útil sólo cuando se interactúa con código C heredado vía `compat`.
#[inline(always)]
pub const fn bx_bool_from_u32(v: u32) -> bx_bool {
    v != 0
}

#[inline(always)]
pub const fn bx_bool_to_u32(v: bx_bool) -> u32 {
    v as u32
}
