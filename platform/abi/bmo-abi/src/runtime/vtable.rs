//! `vtable` -- VTableStore: tabla de interfaces virtuales del BMO ABI.
//!
//! Almacena hasta 64 vtables, cada una con un array de hasta 32 function
//! pointers. Usado por el sistema de interfaces polimorficas entre lenguajes.
//!
//! ## Companions
//!
//! `VTableMethodMeta` adds typed metadata to otherwise opaque `extern "C" fn()`
//! entries: method name, TypeRegistry index for the signature, and flags.

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Capacidad maxima de vtables.
pub const VTABLE_CAP: usize = 64;

/// Maximo numero de metodos por vtable.
pub const VTABLE_METHODS_MAX: usize = 32;

/// Una entrada individual de vtable.
pub type VTableEntry = Option<extern "C" fn()>;

/// Typed metadata for one VTable method.
///
/// Companion to `VTableEntry`. While the entry itself is an opaque function
/// pointer (required for BEF binary compatibility), this struct carries:
/// - The method name (FNV-1a hash)
/// - The TypeRegistry index of the method's `FunctionSignature`
/// - Flags (virtual, final, override, etc.)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VTableMethodMeta {
    /// FNV-1a 64-bit hash of the method name.
    pub name_hash: bx_u64,
    /// Index into TypeRegistry for the method's FunctionSignature.
    pub sig_type_id: bx_u32,
    /// Method flags.
    pub flags: bx_u32,
}

const _: () = assert!(core::mem::size_of::<VTableMethodMeta>() == 16);

impl VTableMethodMeta {
    pub const fn new(name_hash: bx_u64, sig_type_id: bx_u32) -> Self {
        Self {
            name_hash,
            sig_type_id,
            flags: 0,
        }
    }
}

/// Method flags.
pub mod method_flags {
    use crate::bmo_abi::primitives::bx_u32;
    /// Method is virtual (dispatched through vtable).
    pub const VIRTUAL: bx_u32 = 1 << 0;
    /// Method is final (cannot be overridden).
    pub const FINAL: bx_u32 = 1 << 1;
    /// Method overrides a parent method.
    pub const OVERRIDE: bx_u32 = 1 << 2;
    /// Method is pure virtual (abstract, no implementation).
    pub const ABSTRACT: bx_u32 = 1 << 3;
    /// Method is a syscall stub.
    pub const SYSCALL: bx_u32 = 1 << 4;
}

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

/// Almacen de vtables con capacidad fija.
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
        self.vtables
            .get(idx as usize)
            .filter(|_| (idx as usize) < self.count)
    }

    pub fn lookup(&self, interface_id: bx_u64) -> Option<&VTable> {
        self.vtables[..self.count]
            .iter()
            .find(|v| v.interface_id == interface_id)
    }

    pub fn count(&self) -> usize {
        self.count
    }
}
