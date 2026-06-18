//! `BmoVTable` — tabla de despacho canónica BMO.
//!
//! Layout: `VTableHeader` seguido de N `VTableEntry` contiguos.
//! Direccionable directo por índice O(1) — sin búsqueda.

use crate::bmo_abi::primitives::{bx_u16, bx_u32};
use crate::bmo_abi::type_system::TypeId;
use crate::bmo_abi::vtable::entry::VTableEntry;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VTableHeader {
    /// Magic `b"BVT1"` para detección.
    pub magic: bx_u32,
    /// Cantidad de entradas.
    pub n_entries: bx_u16,
    /// Versión schema (incremento monótono).
    pub version: bx_u16,
    /// Tipo concreto que implementa esta vtable.
    pub concrete_type: TypeId,
    /// Tipo de la interfaz (trait/abstract base).
    pub interface_type: TypeId,
}

pub const VTABLE_MAGIC: bx_u32 = u32::from_le_bytes(*b"BVT1");

#[repr(C)]
pub struct BmoVTable<'a> {
    pub header: VTableHeader,
    pub entries: &'a [VTableEntry],
}

impl<'a> BmoVTable<'a> {
    #[inline(always)]
    pub const fn new(
        concrete: TypeId,
        interface: TypeId,
        entries: &'a [VTableEntry],
    ) -> Self {
        Self {
            header: VTableHeader {
                magic: VTABLE_MAGIC,
                n_entries: entries.len() as bx_u16,
                version: 1,
                concrete_type: concrete,
                interface_type: interface,
            },
            entries,
        }
    }

    /// Acceso O(1) por índice. Retorna `None` si índice fuera de rango.
    #[inline(always)]
    pub fn get(&self, idx: usize) -> Option<&VTableEntry> {
        self.entries.get(idx)
    }

    /// Búsqueda O(n) por nombre-hash. Para casos sin índice estable.
    pub fn find(&self, name_hash: bx_u32) -> Option<&VTableEntry> {
        self.entries.iter().find(|e| e.name_hash == name_hash)
    }

    #[inline(always)]
    pub const fn is_valid(&self) -> bool {
        self.header.magic == VTABLE_MAGIC
    }
}
