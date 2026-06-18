//! `BmoStr` y `BmoString` — strings UTF-8 con longitud explícita.
//!
//! Comparativa contra C:
//!
//! | Op             | C (`char*`)            | BMO (`BmoStr`)          |
//! |----------------|------------------------|-------------------------|
//! | length         | `strlen` O(n)          | `.len()` O(1)           |
//! | bounds check   | nunca                  | siempre                 |
//! | nul-terminator | sí (caracter útil perdido) | no                  |
//! | ABI size       | 8 B (puntero)          | 16 B (ptr + len)        |
//! | encoding       | "depende"              | UTF-8 garantizado       |

extern crate alloc;

use core::marker::PhantomData;
use crate::bmo_abi::primitives::{bx_u8, bx_usize};

/// String slice — vista borrow de UTF-8.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoStr<'a> {
    pub ptr: *const bx_u8,
    pub len: bx_usize,
    _marker: PhantomData<&'a str>,
}

impl<'a> BmoStr<'a> {
    pub const EMPTY: Self = Self {
        ptr: core::ptr::null(),
        len: 0,
        _marker: PhantomData,
    };

    #[inline(always)]
    pub const fn from_str(s: &'a str) -> Self {
        Self { ptr: s.as_ptr(), len: s.len() as bx_usize, _marker: PhantomData }
    }

    /// SAFETY: `ptr` debe ser UTF-8 válido y vivir durante `'a`.
    #[inline(always)]
    pub const unsafe fn from_raw(ptr: *const bx_u8, len: bx_usize) -> Self {
        Self { ptr, len, _marker: PhantomData }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &'a str {
        if self.len == 0 { return ""; }
        // SAFETY: invariante de construcción (UTF-8 válido).
        unsafe {
            let bytes = core::slice::from_raw_parts(self.ptr, self.len as usize);
            core::str::from_utf8_unchecked(bytes)
        }
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool { self.len == 0 }

    /// Compara con otro `BmoStr` byte-a-byte (UTF-8 implica equivalencia
    /// canónica si ambos están en NFC). Más rápido que `strcmp` por usar len.
    pub fn eq_str(&self, other: &BmoStr<'_>) -> bool {
        if self.len != other.len { return false; }
        if self.len == 0 { return true; }
        // SAFETY: invariante.
        unsafe {
            let a = core::slice::from_raw_parts(self.ptr, self.len as usize);
            let b = core::slice::from_raw_parts(other.ptr, other.len as usize);
            a == b
        }
    }
}

/// String owned — heap-allocated UTF-8.
///
/// Equivalente a `String` de Rust pero con layout C-FFI y semántica BMO.
#[derive(Debug, Clone)]
pub struct BmoString {
    inner: alloc::vec::Vec<u8>,
}

impl BmoString {
    pub const fn new() -> Self {
        Self { inner: alloc::vec::Vec::new() }
    }

    pub fn from_str(s: &str) -> Self {
        Self { inner: s.as_bytes().to_vec() }
    }

    pub fn push_str(&mut self, s: &str) {
        self.inner.extend_from_slice(s.as_bytes());
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: solo escribimos UTF-8 vía `push_str`.
        unsafe { core::str::from_utf8_unchecked(&self.inner) }
    }

    pub fn as_bmo_str(&self) -> BmoStr<'_> {
        BmoStr::from_str(self.as_str())
    }

    pub fn len(&self) -> usize { self.inner.len() }
    pub fn is_empty(&self) -> bool { self.inner.is_empty() }
}

impl Default for BmoString {
    fn default() -> Self { Self::new() }
}
