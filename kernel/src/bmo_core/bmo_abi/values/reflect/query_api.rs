//! API de queries reflectivas. v1.7.9: stub.

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::bx_u32;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectError {
    NoSuchType   = 1,
    NoSuchField  = 2,
    NoSuchMethod = 3,
    NoMetadata   = 4,
}

pub type ReflectResult<T> = core::result::Result<T, ReflectError>;

/// Query reflectiva. v1.7.9: stub. v2.0: alimentada por `bmo_abi::runtime`.
pub struct ReflectQuery<'a> {
    _phantom: core::marker::PhantomData<&'a ()>,
}

impl<'a> ReflectQuery<'a> {
    pub const fn empty() -> Self {
        Self { _phantom: core::marker::PhantomData }
    }
    pub fn count(&self) -> bx_u32 { 0 }
}
