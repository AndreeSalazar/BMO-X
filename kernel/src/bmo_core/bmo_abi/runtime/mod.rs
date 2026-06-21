//! `runtime` — agregador único de las sub-registries del BMO ABI.
//!
//! Contiene el registro de tipos, vtable storage y el puente FFI entre
//! lenguajes. Cada sub-registry es un módulo separado pero el `BmoRuntime`
//! los agrega en una sola estructura.

#![allow(dead_code)]

pub mod types;
pub mod vtable;
pub mod lang_bridge;

use crate::bmo_core::bmo_abi::primitives::bx_u32;
use types::TypeRegistry;
use vtable::VTableStore;
use lang_bridge::LangBridge;

pub const BMO_RUNTIME_VERSION: bx_u32 = 2;

pub struct BmoRuntime<'a> {
    pub types: TypeRegistry<'a>,
    pub vtables: VTableStore,
    pub lang_bridge: LangBridge,
}

impl<'a> BmoRuntime<'a> {
    pub const fn new() -> Self {
        Self {
            types: TypeRegistry::new(),
            vtables: VTableStore::new(),
            lang_bridge: LangBridge::new(),
        }
    }

    pub const fn version(&self) -> bx_u32 {
        BMO_RUNTIME_VERSION
    }
}

impl<'a> Default for BmoRuntime<'a> {
    fn default() -> Self {
        Self::new()
    }
}
