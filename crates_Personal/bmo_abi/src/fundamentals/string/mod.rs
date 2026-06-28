//! `string` — BmoStr y BmoString, strings UTF-8 del BMO ABI.
//!
//! Reemplaza `char*` null-terminated con una pareja (ptr, len) que elimina
//! las vulnerabilidades clásicas de C (buffer over-read, string truncation).
//!
//! - `BmoStr` — vista prestada (ptr + len), similar a `&str`.
//! - `BmoString` — owned (ptr + len + capacity), similar a `String`.

use alloc::string::String;
use crate::bmo_abi::primitives::bx_u64;

// ─── BmoStr: string prestado (borrowed) ────────────────────────────

/// Vista prestada de una secuencia UTF-8. FFI-safe: (ptr, len).
///
/// # Layout (16 bytes)
/// ```text
/// [0..7]  ptr: *const u8
/// [8..15] len: u64
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoStr {
    ptr: *const u8,
    len: bx_u64,
}

impl BmoStr {
    /// Create from a raw pointer and length.
    ///
    /// # Safety
    /// `ptr` must point to `len` valid bytes of UTF-8 text.
    pub const unsafe fn from_raw(ptr: *const u8, len: bx_u64) -> Self {
        Self { ptr, len }
    }

    /// Create from a `&str`.
    pub fn from_str(s: &str) -> Self {
        Self { ptr: s.as_ptr(), len: s.len() as bx_u64 }
    }

    /// Create from a Rust `&[u8]` (must be valid UTF-8).
    ///
    /// # Safety
    /// Caller ensures bytes are valid UTF-8.
    pub unsafe fn from_bytes_unchecked(bytes: &[u8]) -> Self {
        Self { ptr: bytes.as_ptr(), len: bytes.len() as bx_u64 }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len as usize) }
    }

    /// Interpret as a `&str`.
    ///
    /// # Safety
    /// The underlying bytes must be valid UTF-8.
    pub unsafe fn as_str(&self) -> &str {
        core::str::from_utf8_unchecked(self.as_slice())
    }

    pub const fn len(&self) -> bx_u64 { self.len }
    pub const fn is_empty(&self) -> bool { self.len == 0 }
    pub const fn as_ptr(&self) -> *const u8 { self.ptr }

    /// Compare with a `&str` (for convenience).
    pub fn eq_str(&self, other: &str) -> bool {
        self.as_slice() == other.as_bytes()
    }
}

// ─── BmoString: string owned ───────────────────────────────────────

/// String owned del BMO ABI. FFI-safe: (ptr, len, capacity).
///
/// # Layout (24 bytes)
/// ```text
/// [0..7]  ptr:      *mut u8
/// [8..15] len:      u64
/// [16..23] capacity: u64
/// ```
#[repr(C)]
#[derive(Debug)]
pub struct BmoString {
    ptr: *mut u8,
    len: bx_u64,
    capacity: bx_u64,
}

impl BmoString {
    pub fn new() -> Self {
        Self { ptr: core::ptr::null_mut(), len: 0, capacity: 0 }
    }

    /// Create from a Rust `String`, transferring ownership.
    pub fn from_string(s: String) -> Self {
        let bytes = s.into_bytes();
        let ptr = bytes.as_ptr() as *mut u8;
        let len = bytes.len() as bx_u64;
        let capacity = bytes.capacity() as bx_u64;
        core::mem::forget(bytes);
        Self { ptr, len, capacity }
    }

    /// Create a `BmoString` from raw components. Takes ownership of `ptr`.
    ///
    /// # Safety
    /// `ptr` must have been allocated with the global allocator and have
    /// `capacity` bytes available, of which `len` are initialized UTF-8.
    pub unsafe fn from_raw(ptr: *mut u8, len: bx_u64, capacity: bx_u64) -> Self {
        Self { ptr, len, capacity }
    }

    pub fn as_bmo_str(&self) -> BmoStr {
        unsafe { BmoStr::from_raw(self.ptr, self.len) }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len as usize) }
    }

    pub unsafe fn as_str(&self) -> &str {
        core::str::from_utf8_unchecked(self.as_slice())
    }

    pub const fn len(&self) -> bx_u64 { self.len }
    pub const fn capacity(&self) -> bx_u64 { self.capacity }
    pub const fn is_empty(&self) -> bool { self.len == 0 }
    pub const fn as_ptr(&self) -> *const u8 { self.ptr }
    pub fn as_mut_ptr(&mut self) -> *mut u8 { self.ptr }

    /// Drop the owned buffer (call when BMO ABI callee is done with it).
    ///
    /// # Safety
    /// Must only be called once. After this, the string is invalid.
    pub unsafe fn drop(&mut self) {
        if !self.ptr.is_null() {
            let layout = core::alloc::Layout::from_size_align_unchecked(
                self.capacity as usize, 1);
            alloc::alloc::dealloc(self.ptr, layout);
        }
        self.ptr = core::ptr::null_mut();
        self.len = 0;
        self.capacity = 0;
    }
}

impl Drop for BmoString {
    fn drop(&mut self) {
        unsafe { self.drop(); }
    }
}

// ─── Constants ─────────────────────────────────────────────────────

impl BmoStr {
    pub const EMPTY: Self = Self { ptr: core::ptr::null(), len: 0 };
}

impl BmoString {
    pub const EMPTY: Self = Self {
        ptr: core::ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
}
