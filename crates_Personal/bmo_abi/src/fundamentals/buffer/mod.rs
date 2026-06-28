//! `buffer` — BmoBuffer, descriptor de memoria compartida del BMO ABI.
//!
//! Reemplaza el caos de `void* + size_t` en IPC con un solo tipo que
//! describe una región de memoria compartida entre procesos/contenedores.
//!
//! # Layout (32 bytes)
//! ```text
//! [0..7]  ptr:      *mut u8  — dirección virtual
//! [8..15] len:      u64      — tamaño en bytes
//! [16..23] capacity: u64     — capacidad total (≥ len)
//! [24..31] flags:    u64     — BmoBufferFlags
//! ```

use crate::bmo_abi::primitives::bx_u64;

bitflags::bitflags! {
    /// Flags de un `BmoBuffer`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BmoBufferFlags: bx_u64 {
        /// El buffer es de solo lectura para el receptor.
        const READ_ONLY  = 1 << 0;
        /// El buffer se puede redimensionar.
        const RESIZABLE  = 1 << 1;
        /// El buffer debe estar alineado a página.
        const PAGE_ALIGN = 1 << 2;
        /// El buffer es persistente (no se libera al cerrar handle).
        const PERSISTENT = 1 << 3;
        /// El buffer es mapeado como no-caché (para device DMA).
        const UNCACHED   = 1 << 4;
    }
}

/// Descriptor de memoria compartida.
///
/// 32 bytes, pasa por valor en registros o en un bloque de memoria.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoBuffer {
    pub ptr: *mut u8,
    pub len: bx_u64,
    pub capacity: bx_u64,
    pub flags: BmoBufferFlags,
}
const _: () = assert!(core::mem::size_of::<BmoBuffer>() == 32);

impl BmoBuffer {
    pub const EMPTY: Self = Self {
        ptr: core::ptr::null_mut(),
        len: 0,
        capacity: 0,
        flags: BmoBufferFlags::empty(),
    };

    pub const fn new(ptr: *mut u8, len: bx_u64, capacity: bx_u64, flags: BmoBufferFlags) -> Self {
        Self { ptr, len, capacity, flags }
    }

    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            ptr: slice.as_ptr() as *mut u8,
            len: slice.len() as bx_u64,
            capacity: slice.len() as bx_u64,
            flags: BmoBufferFlags::READ_ONLY,
        }
    }

    pub fn from_slice_mut(slice: &mut [u8]) -> Self {
        Self {
            ptr: slice.as_mut_ptr(),
            len: slice.len() as bx_u64,
            capacity: slice.len() as bx_u64,
            flags: BmoBufferFlags::empty(),
        }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub fn as_slice(&self) -> &[u8] {
        if self.ptr.is_null() {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(self.ptr, self.len as usize) }
        }
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        if self.ptr.is_null() {
            &mut []
        } else {
            unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len as usize) }
        }
    }

    pub const fn len(&self) -> bx_u64 { self.len }
    pub const fn capacity(&self) -> bx_u64 { self.capacity }
    pub const fn is_empty(&self) -> bool { self.len == 0 }
}
