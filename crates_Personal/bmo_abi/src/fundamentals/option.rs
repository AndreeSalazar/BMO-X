//! `option` — `BmoOption<T>` FFI-safe.
//!
//! El `Option<T>` de Rust es niche-optimized solo para algunos tipos
//! (referencias, `NonZero*`, etc.). Para FFI estable necesitamos un
//! layout C explícito.
//!
//! `BmoOption<T>` empaca `(u32 tag, T value)` con repr(C). Para `T` que
//! cabe en 8 bytes, total 16 B → 2 GPRs en BMO ABI.

#![allow(dead_code)]

use crate::bmo_abi::primitives::bx_u32;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoOption<T: Copy> {
    pub tag: bx_u32,        // 0 = None, 1 = Some
    pub _pad: bx_u32,
    pub value: T,           // válido solo si tag == 1
}

impl<T: Copy + Default> BmoOption<T> {
    #[inline(always)]
    pub fn none() -> Self {
        Self { tag: 0, _pad: 0, value: T::default() }
    }

    #[inline(always)]
    pub const fn some(v: T) -> Self {
        Self { tag: 1, _pad: 0, value: v }
    }

    #[inline(always)]
    pub const fn is_some(&self) -> bool { self.tag == 1 }

    #[inline(always)]
    pub const fn is_none(&self) -> bool { self.tag == 0 }

    #[inline(always)]
    pub fn unwrap_or(self, default: T) -> T {
        if self.is_some() { self.value } else { default }
    }

    /// Convierte a `Option<T>` de Rust para uso ergonómico interno.
    #[inline(always)]
    pub fn into_option(self) -> Option<T> {
        if self.is_some() { Some(self.value) } else { None }
    }

    #[inline(always)]
    pub fn from_option(o: Option<T>) -> Self {
        match o {
            Some(v) => Self::some(v),
            None    => Self::none(),
        }
    }
}
