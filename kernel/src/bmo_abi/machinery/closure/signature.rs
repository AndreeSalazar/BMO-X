//! `ClosureSig` — signature de un closure BMO (params + return).

use crate::bmo_abi::type_system::TypeId;
use crate::bmo_abi::primitives::bx_u8;

/// Multiplicidad de invocación del closure.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosureKind {
    /// Llamable múltiples veces sin mutar entorno (`Fn` de Rust).
    Pure   = 0,
    /// Llamable múltiples veces, muta entorno (`FnMut`).
    Mut    = 1,
    /// Llamable una sola vez, consume entorno (`FnOnce`).
    Once   = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ClosureSig {
    pub kind: ClosureKind,
    pub n_params: bx_u8,
    pub _pad: [u8; 6],
    pub params: [TypeId; 8], // hasta 8 parámetros inline; más → spill table
    pub return_type: TypeId,
}

impl ClosureSig {
    pub const VOID_TO_VOID: Self = Self {
        kind: ClosureKind::Pure,
        n_params: 0,
        _pad: [0; 6],
        params: [TypeId::VOID; 8],
        return_type: TypeId::VOID,
    };
}
