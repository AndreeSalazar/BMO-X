//! `bx_bool` -- bool canonico del BMO ABI. Reemplaza `<stdbool.h>`.
//!
//! 1 byte (vs el `BOOL` 32-bit de Win32 que desperdicia 24 bits). Same
//! representation que el `bool` de Rust -> conversiones gratis.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  VERDE        `bx_bool` para cruzar la frontera FFI
//! [cuesta]  NADA         no hay nada detras
//! [riesgo]  SILENCIO     un booleano que cruza como entero admite valores
//!                        que no son 0 ni 1

#![allow(non_camel_case_types)]

pub type bx_bool = bool;

pub const BX_TRUE: bx_bool = true;
pub const BX_FALSE: bx_bool = false;

/// Helper: convierte un `bx_u32` (estilo Win32 `BOOL`) a `bx_bool`.
/// Util solo cuando se interactua con codigo C heredado via `compat`.
#[inline(always)]
pub const fn bx_bool_from_u32(v: u32) -> bx_bool {
    v != 0
}

#[inline(always)]
pub const fn bx_bool_to_u32(v: bx_bool) -> u32 {
    v as u32
}
