//! Cache LRU de shaders ya traducidos. Key = BLAKE3 del blob origen.
//!
//! Evita re-traducir DXIL/SPIR-V a IR/backend nativo cada vez que la misma app abre
//! el mismo PSO. Análogo al D3D12 PSO cache pero por shader individual.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::{bx_u32, bx_u64};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CacheEntry {
    /// Primeros 8 bytes del BLAKE3 del blob origen.
    pub key: bx_u64,
    /// Handle del shader ya cargado en el device.
    pub handle: bx_u32,
    /// Bytes del blob traducido resultante (para reconstruir si se descarga).
    pub native_size: bx_u32,
}

pub struct ShaderCache<'a> {
    pub entries: &'a mut [CacheEntry],
    pub used: bx_u32,
}

impl<'a> ShaderCache<'a> {
    pub const fn from_slice(entries: &'a mut [CacheEntry]) -> Self {
        Self { entries, used: 0 }
    }

    pub fn lookup(&self, _key: bx_u64) -> Option<bx_u32> {
        None
    }

    pub fn insert(&mut self, _entry: CacheEntry) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
