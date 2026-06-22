//! `result` — `BmoResult<T>` FFI-safe genérico sobre el tipo de éxito.
//!
//! Diferencia con `BmoStatus`:
//!   - `BmoStatus` siempre lleva `value: u64` (handle/contador).
//!   - `BmoResult<T>` lleva el tipo `T` real cuando `Ok`.
//!
//! Útil cuando una función necesita devolver un valor de tipo concreto
//! (`u32`, struct específica) en lugar del wire format universal.

#![allow(dead_code)]

use crate::bmo_abi::primitives::bx_u32;
use crate::bmo_abi::status::ErrorCode;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoResult<T: Copy> {
    /// Código de error (0 = Ok). Cuando es 0, `value` es válido; en otro
    /// caso, su contenido es indeterminado.
    pub code: bx_u32,
    pub _pad: bx_u32,
    pub value: T,
}

impl<T: Copy + Default> BmoResult<T> {
    #[inline(always)]
    pub const fn ok(v: T) -> Self {
        Self { code: ErrorCode::OK, _pad: 0, value: v }
    }

    #[inline(always)]
    pub fn err(code: bx_u32) -> Self {
        Self { code, _pad: 0, value: T::default() }
    }

    #[inline(always)]
    pub const fn is_ok(&self) -> bool { self.code == ErrorCode::OK }

    #[inline(always)]
    pub const fn is_err(&self) -> bool { self.code != ErrorCode::OK }

    #[inline(always)]
    pub fn unwrap_or(self, default: T) -> T {
        if self.is_ok() { self.value } else { default }
    }

    /// Convierte a `Result<T, u32>` de Rust.
    #[inline(always)]
    pub fn into_result(self) -> Result<T, bx_u32> {
        if self.is_ok() { Ok(self.value) } else { Err(self.code) }
    }

    #[inline(always)]
    pub fn from_result(r: Result<T, bx_u32>) -> Self {
        match r {
            Ok(v)    => Self::ok(v),
            Err(c)   => Self::err(c),
        }
    }
}
