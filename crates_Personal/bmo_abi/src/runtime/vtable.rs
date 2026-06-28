//! `vtable` — VTableStore: tabla de interfaces virtuales del BMO ABI.
//!
//! Almacena hasta 64 vtables, cada una con un array de hasta 32 function
//! pointers. Usado por el sistema de interfaces polimórficas entre lenguajes.

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Capacidad máxima de vtables.
pub const VTABLE_CAP: usize = 64;

/// Máximo número de métodos por vtable.
pub const VTABLE_METHODS_MAX: usize = 32;

/// Una entrada individual de vtable.
pub type VTableEntry = Option<extern "C" fn()>;

/// Una vtable: array de function pointers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VTable {
    pub methods: [VTableEntry; VTABLE_METHODS_MAX],
    pub method_count: bx_u32,
    pub interface_id: bx_u64,
}
const _: () = assert!(core::mem::size_of::<VTable>() == 272);

impl VTable {
    pub const fn empty() -> Self {
        Self {
            methods: [None; VTABLE_METHODS_MAX],
            method_count: 0,
            interface_id: 0,
        }
    }
}

/// Almacén de vtables con capacidad fija.
pub struct VTableStore {
    vtables: [VTable; VTABLE_CAP],
    count: usize,
}

impl VTableStore {
    pub const fn new() -> Self {
        Self {
            vtables: [VTable::empty(); VTABLE_CAP],
            count: 0,
        }
    }

    pub fn register(&mut self, vtable: VTable) -> Option<bx_u32> {
        if self.count >= VTABLE_CAP {
            return None;
        }
        let idx = self.count as bx_u32;
        self.vtables[self.count] = vtable;
        self.count += 1;
        Some(idx)
    }

    pub fn get(&self, idx: bx_u32) -> Option<&VTable> {
        self.vtables.get(idx as usize).filter(|_| (idx as usize) < self.count)
    }

    pub fn lookup(&self, interface_id: bx_u64) -> Option<&VTable> {
        self.vtables[..self.count].iter().find(|v| v.interface_id == interface_id)
    }

    pub fn count(&self) -> usize { self.count }
}
