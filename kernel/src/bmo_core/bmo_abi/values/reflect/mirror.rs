//! `Mirror` — vista reflectiva sobre un valor o tipo.
//!
//! v1.7.9: stub. Inspirada en los Mirrors de Strongtalk/Smalltalk.
//! En v2.0 se llenará con datos de `bmo_abi::runtime::types`.

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::bx_u64;

/// Mirror placeholder. v2.0: contendrá un descriptor real.
#[derive(Debug, Clone, Copy)]
pub struct Mirror<'a> {
    pub type_name: &'a [u8],
}

impl<'a> Mirror<'a> {
    pub const fn new(type_name: &'a [u8]) -> Self {
        Self { type_name }
    }
    pub fn type_name(&self) -> &[u8] { self.type_name }
}

/// Mirror sobre un *valor* concreto.
#[derive(Debug, Clone, Copy)]
pub struct MirrorOf<'a> {
    pub mirror: Mirror<'a>,
    pub value_ptr: bx_u64,
}

impl<'a> MirrorOf<'a> {
    pub const fn new(type_name: &'a [u8], value_ptr: bx_u64) -> Self {
        Self { mirror: Mirror::new(type_name), value_ptr }
    }
}
