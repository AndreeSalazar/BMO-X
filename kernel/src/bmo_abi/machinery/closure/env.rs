//! `ClosureEnv` — entorno capturado de un closure.
//!
//! Bloque opaco con tipo (`TypeId`) cuyo layout describe `type_system`.

use crate::bmo_abi::primitives::{bx_u32, bx_u64};
use crate::bmo_abi::type_system::TypeId;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ClosureEnv {
    /// TypeId del struct anónimo del entorno.
    pub env_type: TypeId,
    /// Puntero al bloque (heap o pila si vive lo suficiente).
    pub data_ptr: bx_u64,
    /// Tamaño del bloque (redundante con `env_type` pero acelera drop).
    pub size: bx_u32,
    /// 0 = entorno en pila (no liberar), 1 = heap (liberar al drop).
    pub heap_owned: bx_u32,
}

impl ClosureEnv {
    pub const EMPTY: Self = Self {
        env_type: TypeId::VOID,
        data_ptr: 0,
        size: 0,
        heap_owned: 0,
    };

    #[inline(always)]
    pub const fn is_empty(&self) -> bool { self.size == 0 }
}
