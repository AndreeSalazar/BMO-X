//! `Mirror` — vista reflectiva sobre un valor o tipo.

#![allow(dead_code)]

use crate::bmo_abi::primitives::bx_u64;
use crate::bmo_abi::values::reflect::{TypeDescriptor, TypeKind};

#[derive(Debug, Clone, Copy)]
pub struct Mirror<'a> {
    descriptor: &'a TypeDescriptor<'a>,
}

impl<'a> Mirror<'a> {
    pub const fn new(descriptor: &'a TypeDescriptor<'a>) -> Self {
        Self { descriptor }
    }

    pub fn type_name(&self) -> &'a str {
        self.descriptor.name.as_str()
    }

    pub fn size(&self) -> bx_u64 {
        self.descriptor.size
    }

    pub fn align(&self) -> bx_u64 {
        self.descriptor.align
    }

    pub fn kind(&self) -> TypeKind {
        self.descriptor.kind
    }

    pub const fn is_primitive(&self) -> bool {
        matches!(self.descriptor.kind, TypeKind::Primitive)
    }

    pub const fn is_struct(&self) -> bool {
        matches!(self.descriptor.kind, TypeKind::Struct)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MirrorOf<'a> {
    pub mirror: Mirror<'a>,
    pub value_ptr: bx_u64,
}

impl<'a> MirrorOf<'a> {
    pub const fn new(descriptor: &'a TypeDescriptor<'a>, value_ptr: bx_u64) -> Self {
        Self { mirror: Mirror::new(descriptor), value_ptr }
    }
}
