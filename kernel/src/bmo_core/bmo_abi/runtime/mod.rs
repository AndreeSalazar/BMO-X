//! `bmo_abi::runtime` — agregador único de las sub-registries del BMO ABI.
//!
//! v1.7.9: simplificado a un stub. Las registries específicas (types,
//! vtables, lang_bridge, exceptions) se manejan en módulos separados
//! en cada idioma de BMO. v2.0 reintroducirá el agregador cuando haya
//! un caso real de uso.

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::bx_u32;

/// Versión del runtime (placeholder).
pub const BMO_RUNTIME_VERSION: bx_u32 = 1;

/// Runtime placeholder. En v2.0 contendrá types, vtables, lang_bridge,
/// unwind, etc. Por ahora solo el número de versión.
#[derive(Debug, Clone, Copy)]
pub struct BmoRuntimePlaceholder {
    pub version: bx_u32,
}

impl BmoRuntimePlaceholder {
    pub const EMPTY: Self = Self { version: 1 };

    pub fn new() -> Self { Self { version: 1 } }

    pub fn version(&self) -> bx_u32 { self.version }
}

impl Default for BmoRuntimePlaceholder {
    fn default() -> Self { Self::new() }
}
