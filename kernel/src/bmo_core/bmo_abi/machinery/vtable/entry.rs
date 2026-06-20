//! `VTableEntry` — slot único en una vtable BMO.

use crate::bmo_core::bmo_abi::primitives::{bx_u32, bx_u64};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Puntero a función con signature BMO ABI.
    Method        = 0,
    /// Slot reservado / abstract method (llamada → `BxError::NotImplemented`).
    Abstract      = 1,
    /// Re-exporta una entrada de un padre (single inheritance).
    Inherited     = 2,
    /// Punto de query a otra interfaz (substituye `QueryInterface`).
    InterfaceLink = 3,
    /// Destructor / finalizador.
    Destructor    = 4,
    /// Dummy / padding.
    Padding       = 0xFF,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VTableEntry {
    pub kind: EntryKind,
    pub _pad: [u8; 3],
    /// Hash 32-bit del nombre del método (FNV-1a estable).
    pub name_hash: bx_u32,
    /// Puntero a código (o 0 si abstract).
    pub fn_ptr: bx_u64,
}

impl VTableEntry {
    pub const ABSTRACT: Self = Self {
        kind: EntryKind::Abstract,
        _pad: [0; 3],
        name_hash: 0,
        fn_ptr: 0,
    };

    #[inline(always)]
    pub const fn method(name_hash: bx_u32, fn_ptr: bx_u64) -> Self {
        Self { kind: EntryKind::Method, _pad: [0; 3], name_hash, fn_ptr }
    }

    #[inline(always)]
    pub const fn is_callable(&self) -> bool {
        self.fn_ptr != 0
    }
}
