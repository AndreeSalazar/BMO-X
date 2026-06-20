//! Slices del BMO ABI: `(ptr, len)` empacados en 16 bytes.
//!
//! Reemplaza el patrón C `void* buf, size_t len` que sufre de:
//!   - orden inconsistente (a veces `(ptr, len)`, a veces `(len, ptr)`)
//!   - cero verificación de bounds en runtime
//!   - imposibilidad de pasar slices "vacíos pero no-null"
//!
//! Layout: idéntico al `&[u8]` fat pointer de Rust, **pero `#[repr(C)]`**
//! para FFI estable. Los dos campos consecutivos caben perfectamente en
//! `RDI:RSI` o cualquier par de GPRs del BMO ABI.

use core::marker::PhantomData;
use crate::bmo_core::bmo_abi::primitives::{bx_u8, bx_usize};

/// Slice inmutable: `(ptr, len)`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoSlice<'a> {
    pub ptr: *const bx_u8,
    pub len: bx_usize,
    _marker: PhantomData<&'a [bx_u8]>,
}

impl<'a> BmoSlice<'a> {
    pub const EMPTY: Self = Self {
        ptr: core::ptr::null(),
        len: 0,
        _marker: PhantomData,
    };

    #[inline(always)]
    pub const fn from_bytes(bytes: &'a [u8]) -> Self {
        Self { ptr: bytes.as_ptr(), len: bytes.len() as bx_usize, _marker: PhantomData }
    }

    #[inline(always)]
    pub const fn from_str(s: &'a str) -> Self {
        Self { ptr: s.as_ptr(), len: s.len() as bx_usize, _marker: PhantomData }
    }

    /// SAFETY: Caller garantiza que `ptr` es válido para `len` bytes durante `'a`.
    #[inline(always)]
    pub const unsafe fn from_raw(ptr: *const bx_u8, len: bx_usize) -> Self {
        Self { ptr, len, _marker: PhantomData }
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &'a [u8] {
        if self.len == 0 { return &[]; }
        // SAFETY: invariante estructural de BmoSlice.
        unsafe { core::slice::from_raw_parts(self.ptr, self.len as usize) }
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool { self.len == 0 }
}

/// Slice mutable: `(ptr, len)` con permiso de escritura.
#[repr(C)]
#[derive(Debug)]
pub struct BmoMutSlice<'a> {
    pub ptr: *mut bx_u8,
    pub len: bx_usize,
    _marker: PhantomData<&'a mut [bx_u8]>,
}

impl<'a> BmoMutSlice<'a> {
    pub const EMPTY: Self = Self {
        ptr: core::ptr::null_mut(),
        len: 0,
        _marker: PhantomData,
    };

    #[inline(always)]
    pub fn from_mut_bytes(bytes: &'a mut [u8]) -> Self {
        Self {
            ptr: bytes.as_mut_ptr(),
            len: bytes.len() as bx_usize,
            _marker: PhantomData,
        }
    }

    /// SAFETY: Caller garantiza alias-freedom durante `'a`.
    #[inline(always)]
    pub const unsafe fn from_raw(ptr: *mut bx_u8, len: bx_usize) -> Self {
        Self { ptr, len, _marker: PhantomData }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &'a mut [u8] {
        if self.len == 0 { return &mut []; }
        // SAFETY: invariante estructural.
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len as usize) }
    }
}
