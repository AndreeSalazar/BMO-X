#![allow(dead_code)]

use crate::bmo_core::bmo_abi::fundamentals::handle::BmoHandle;
use crate::bmo_core::bmo_abi::fundamentals::handle::kind::HandleKind;
use crate::bmo_core::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64};

const MAX_VTABLE_SLOTS: usize = 64;
const MAX_METHODS: usize = 16;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VTableEntry {
    pub type_handle: BmoHandle,
    pub method_count: bx_u32,
    pub methods: [bx_u64; MAX_METHODS],
}

impl VTableEntry {
    pub const EMPTY: Self = Self {
        type_handle: BmoHandle::NULL,
        method_count: 0,
        methods: [0; MAX_METHODS],
    };
}

pub struct VTableStore {
    entries: [VTableEntry; MAX_VTABLE_SLOTS],
    count: bx_u32,
    generations: [bx_u16; MAX_VTABLE_SLOTS],
}

impl VTableStore {
    pub const fn new() -> Self {
        Self {
            entries: [VTableEntry::EMPTY; MAX_VTABLE_SLOTS],
            count: 0,
            generations: [0; MAX_VTABLE_SLOTS],
        }
    }

    pub fn register_vtable(&mut self, entry: VTableEntry) -> BmoHandle {
        let idx = self.count as usize;
        if idx >= MAX_VTABLE_SLOTS {
            return BmoHandle::NULL;
        }
        let gen = self.generations[idx];
        self.entries[idx] = entry;
        self.generations[idx] = gen.wrapping_add(1);
        self.count += 1;
        BmoHandle::new(HandleKind::Queue, self.generations[idx], idx as bx_u64)
    }

    pub fn get_vtable(&self, handle: BmoHandle) -> Option<&VTableEntry> {
        let idx = handle.index() as usize;
        let gen = handle.generation();
        if idx >= MAX_VTABLE_SLOTS {
            return None;
        }
        if self.generations[idx] != gen {
            return None;
        }
        Some(&self.entries[idx])
    }

    pub fn count(&self) -> bx_u32 {
        self.count
    }
}

impl Default for VTableStore {
    fn default() -> Self {
        Self::new()
    }
}
