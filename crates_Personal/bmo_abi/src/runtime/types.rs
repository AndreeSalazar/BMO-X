#![allow(dead_code)]

use crate::bmo_abi::fundamentals::handle::BmoHandle;
use crate::bmo_abi::fundamentals::handle::kind::HandleKind;
use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64};
use crate::bmo_abi::values::reflect::TypeDescriptor;

const MAX_TYPE_SLOTS: usize = 256;

pub struct TypeRegistry<'a> {
    descriptors: [Option<TypeDescriptor<'a>>; MAX_TYPE_SLOTS],
    count: bx_u32,
    generations: [bx_u16; MAX_TYPE_SLOTS],
}

impl<'a> TypeRegistry<'a> {
    pub const fn new() -> Self {
        const NONE_DESC: Option<TypeDescriptor<'_>> = None;
        const ZERO_GEN: bx_u16 = 0;
        Self {
            descriptors: [NONE_DESC; MAX_TYPE_SLOTS],
            count: 0,
            generations: [ZERO_GEN; MAX_TYPE_SLOTS],
        }
    }

    pub fn register_type(&mut self, desc: TypeDescriptor<'a>) -> BmoHandle {
        let idx = self.count as usize;
        if idx >= MAX_TYPE_SLOTS {
            return BmoHandle::NULL;
        }
        let gen = self.generations[idx];
        self.descriptors[idx] = Some(desc);
        self.generations[idx] = gen.wrapping_add(1);
        self.count += 1;
        BmoHandle::new(HandleKind::Buffer, self.generations[idx], idx as bx_u64)
    }

    pub fn get_descriptor(&self, handle: BmoHandle) -> Option<&TypeDescriptor<'a>> {
        let idx = handle.index() as usize;
        let gen = handle.generation();
        if idx >= MAX_TYPE_SLOTS {
            return None;
        }
        if self.generations[idx] != gen {
            return None;
        }
        self.descriptors[idx].as_ref()
    }

    pub fn count(&self) -> bx_u32 {
        self.count
    }
}

impl<'a> Default for TypeRegistry<'a> {
    fn default() -> Self {
        Self::new()
    }
}
