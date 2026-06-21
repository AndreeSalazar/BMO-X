//! API de queries reflectivas.

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::fundamentals::handle::BmoHandle;
use crate::bmo_core::bmo_abi::fundamentals::handle::kind::HandleKind;
use crate::bmo_core::bmo_abi::primitives::bx_u32;
use crate::bmo_core::bmo_abi::runtime::types::TypeRegistry;
use crate::bmo_core::bmo_abi::values::reflect::TypeKind;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectError {
    NoSuchType   = 1,
    NoSuchField  = 2,
    NoSuchMethod = 3,
    NoMetadata   = 4,
}

pub type ReflectResult<T> = core::result::Result<T, ReflectError>;

const MAX_RESULTS: usize = 256;

pub struct ReflectQuery<'a> {
    registry: &'a TypeRegistry<'a>,
    results: [BmoHandle; MAX_RESULTS],
    result_count: bx_u32,
}

impl<'a> ReflectQuery<'a> {
    pub fn new(registry: &'a TypeRegistry<'a>) -> Self {
        Self {
            registry,
            results: [BmoHandle::NULL; MAX_RESULTS],
            result_count: 0,
        }
    }

    pub fn find_by_name(&mut self, name: &str) -> Option<BmoHandle> {
        self.result_count = 0;
        let count = self.registry.count();
        let mut i: bx_u32 = 0;
        while i < count {
            let h = BmoHandle::new(HandleKind::Buffer, 0, i as u64);
            if let Some(desc) = self.registry.get_descriptor(h) {
                if desc.name.as_str() == name {
                    return Some(h);
                }
            }
            i += 1;
        }
        None
    }

    pub fn find_by_kind(&mut self, kind: TypeKind) -> bx_u32 {
        self.result_count = 0;
        let count = self.registry.count();
        let mut i: bx_u32 = 0;
        while i < count {
            let h = BmoHandle::new(HandleKind::Buffer, 0, i as u64);
            if let Some(desc) = self.registry.get_descriptor(h) {
                if desc.kind == kind && (self.result_count as usize) < MAX_RESULTS {
                    self.results[self.result_count as usize] = h;
                    self.result_count += 1;
                }
            }
            i += 1;
        }
        self.result_count
    }

    pub fn result_handle(&self, index: bx_u32) -> Option<BmoHandle> {
        if (index as usize) < MAX_RESULTS && index < self.result_count {
            Some(self.results[index as usize])
        } else {
            None
        }
    }

    pub fn count(&self) -> bx_u32 {
        self.registry.count()
    }

    pub fn result_count(&self) -> bx_u32 {
        self.result_count
    }
}
