#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

const MAX_LANG_SLOTS: usize = 8;

pub const HAS_COMPILER: bx_u32 = 1;
pub const HAS_RUNTIME:  bx_u32 = 2;
pub const HAS_FFI:      bx_u32 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LangInfo {
    pub name: bx_u64,
    pub version: bx_u32,
    pub capabilities: bx_u32,
}

impl LangInfo {
    pub const EMPTY: Self = Self { name: 0, version: 0, capabilities: 0 };

    pub const fn new(name: bx_u64, version: bx_u32, capabilities: bx_u32) -> Self {
        Self { name, version, capabilities }
    }

    pub const fn has_compiler(&self) -> bool { (self.capabilities & HAS_COMPILER) != 0 }
    pub const fn has_runtime(&self) -> bool { (self.capabilities & HAS_RUNTIME) != 0 }
    pub const fn has_ffi(&self) -> bool { (self.capabilities & HAS_FFI) != 0 }
}

pub struct LangBridge {
    slots: [LangInfo; MAX_LANG_SLOTS],
    count: bx_u32,
}

impl LangBridge {
    pub const fn new() -> Self {
        Self {
            slots: [LangInfo::EMPTY; MAX_LANG_SLOTS],
            count: 0,
        }
    }

    pub fn register_lang(&mut self, info: LangInfo) -> bx_u32 {
        let id = self.count;
        if id as usize >= MAX_LANG_SLOTS {
            return u32::MAX;
        }
        self.slots[id as usize] = info;
        self.count += 1;
        id
    }

    pub fn get_lang(&self, id: bx_u32) -> Option<&LangInfo> {
        if (id as usize) >= MAX_LANG_SLOTS || id >= self.count {
            return None;
        }
        Some(&self.slots[id as usize])
    }

    pub fn count(&self) -> bx_u32 {
        self.count
    }
}

impl Default for LangBridge {
    fn default() -> Self {
        Self::new()
    }
}
