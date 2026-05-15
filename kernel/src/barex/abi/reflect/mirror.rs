//! `Mirror` — vista reflectiva sobre un valor o tipo.
//!
//! Inspirada en los Mirrors de Strongtalk/Smalltalk: la reflection NO está
//! en el objeto, sino en un objeto separado (`Mirror`) que evita que cada
//! valor BMO cargue overhead.

use crate::barex::abi::primitives::bx_u64;
use crate::barex::abi::type_system::{TypeDescriptor, TypeId};

/// Mirror sobre un *tipo* (estático).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Mirror<'a> {
    pub descriptor: &'a TypeDescriptor<'a>,
}

impl<'a> Mirror<'a> {
    #[inline(always)]
    pub const fn new(descriptor: &'a TypeDescriptor<'a>) -> Self {
        Self { descriptor }
    }

    #[inline(always)]
    pub const fn type_id(&self) -> TypeId { self.descriptor.id }
}

/// Mirror sobre un *valor* concreto.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MirrorOf<'a> {
    pub mirror: Mirror<'a>,
    /// Puntero al valor reflejado.
    pub value_ptr: bx_u64,
}

impl<'a> MirrorOf<'a> {
    #[inline(always)]
    pub const fn new(descriptor: &'a TypeDescriptor<'a>, value_ptr: bx_u64) -> Self {
        Self { mirror: Mirror::new(descriptor), value_ptr }
    }
}
