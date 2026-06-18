//! `BmoFatPtr` — puntero gordo (data + vtable) canónico.
//!
//! 16 bytes, idéntico layout que el `dyn Trait` de Rust pero `repr(C)`
//! para que C++/Java/Swift puedan compartirlo. Cabe en RAX:RDX.

use crate::bmo_abi::primitives::bx_u64;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoFatPtr {
    /// Puntero al objeto concreto.
    pub data: bx_u64,
    /// Puntero a la `BmoVTable` correspondiente.
    pub vtable: bx_u64,
}

impl BmoFatPtr {
    pub const NULL: Self = Self { data: 0, vtable: 0 };

    #[inline(always)]
    pub const fn new(data: bx_u64, vtable: bx_u64) -> Self {
        Self { data, vtable }
    }

    #[inline(always)]
    pub const fn is_null(&self) -> bool {
        self.data == 0 || self.vtable == 0
    }
}
