//! `memory` -- tipos de memoria del BMO ABI.
//!
//! Reemplaza `void* + size_t` de C con tipos que llevan la semantica
//! incorporada: BmoSlice (ptr + len), BmoRange (offset + size), BmoAligned
//! (alineacion garantizada).
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  ROJO         `BmoSlice`, `BmoRange`, `BmoAligned` -- aritmetica
//!                        sobre punteros crudos
//! [cuesta]  MAQUINA      un rango mal calculado es una lectura o escritura
//!                        fuera de lo entregado
//! [riesgo]  AJENO        los limites vienen de fuera; comprobarlos aqui es
//!                        lo unico que hay entre eso y el mapeo

use crate::bmo_abi::primitives::bx_u64;

/// Slice de memoria: puntero + longitud. FFI-safe (16 bytes).
///
/// Reemplaza `void* + size_t` como argumento de funcion.
/// No hay asercion de ownership -- es prestado (borrowed).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoSlice {
    pub ptr: *const u8,
    pub len: bx_u64,
}
const _: () = assert!(core::mem::size_of::<BmoSlice>() == 16);

impl BmoSlice {
    pub const EMPTY: Self = Self {
        ptr: core::ptr::null(),
        len: 0,
    };

    pub const fn new(ptr: *const u8, len: bx_u64) -> Self {
        Self { ptr, len }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            ptr: bytes.as_ptr(),
            len: bytes.len() as bx_u64,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        if self.ptr.is_null() {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(self.ptr, self.len as usize) }
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub const fn len(&self) -> bx_u64 {
        self.len
    }
}

/// Slice mutable: puntero + longitud.
#[repr(C)]
#[derive(Debug)]
pub struct BmoSliceMut {
    pub ptr: *mut u8,
    pub len: bx_u64,
}
const _: () = assert!(core::mem::size_of::<BmoSliceMut>() == 16);

impl BmoSliceMut {
    pub const EMPTY: Self = Self {
        ptr: core::ptr::null_mut(),
        len: 0,
    };

    pub const fn new(ptr: *mut u8, len: bx_u64) -> Self {
        Self { ptr, len }
    }

    pub fn from_bytes(bytes: &mut [u8]) -> Self {
        Self {
            ptr: bytes.as_mut_ptr(),
            len: bytes.len() as bx_u64,
        }
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
}

/// Rango de memoria: offset + tamano.
///
/// Util para operaciones de mapeo, scatter-gather, y acceso a secciones.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoRange {
    pub offset: bx_u64,
    pub size: bx_u64,
}
const _: () = assert!(core::mem::size_of::<BmoRange>() == 16);

impl BmoRange {
    pub const fn new(offset: bx_u64, size: bx_u64) -> Self {
        Self { offset, size }
    }

    pub const fn end(&self) -> bx_u64 {
        self.offset + self.size
    }

    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// True if `other` is fully contained in this range.
    pub const fn contains(&self, other: &BmoRange) -> bool {
        other.offset >= self.offset && other.end() <= self.end()
    }
}

/// Memoria con alineacion garantizada.
///
/// Garantiza que `ptr` cumple `align` bytes de alineacion.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoAligned {
    pub ptr: *mut u8,
    pub align: bx_u64,
}
const _: () = assert!(core::mem::size_of::<BmoAligned>() == 16);

impl BmoAligned {
    pub const fn new(ptr: *mut u8, align: bx_u64) -> Self {
        Self { ptr, align }
    }

    pub fn is_aligned(&self) -> bool {
        (self.ptr as usize) % (self.align as usize) == 0
    }
}

// --- Constantes de alineacion --------------------------------------

pub const BMO_ALIGN_1: bx_u64 = 1;
pub const BMO_ALIGN_2: bx_u64 = 2;
pub const BMO_ALIGN_4: bx_u64 = 4;
pub const BMO_ALIGN_8: bx_u64 = 8;
pub const BMO_ALIGN_16: bx_u64 = 16;
pub const BMO_ALIGN_32: bx_u64 = 32;
pub const BMO_ALIGN_64: bx_u64 = 64;
pub const BMO_ALIGN_128: bx_u64 = 128;
pub const BMO_ALIGN_PAGE: bx_u64 = 4096;
