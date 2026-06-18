//! `TypeLayout` — size + alignment + padding info.
//!
//! Reemplaza `sizeof`/`alignof`/`offsetof` de C. FFI-estable.

use crate::bmo_abi::primitives::{bx_u16, bx_u32};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeLayout {
    /// Tamaño en bytes (incluyendo padding interno y final).
    pub size: bx_u32,
    /// Alineación requerida (potencia de 2: 1, 2, 4, 8, 16, 32, 64).
    pub align: bx_u16,
    /// Flags de layout (ver [`LayoutFlags`]).
    pub flags: bx_u16,
}

impl TypeLayout {
    pub const ZST: Self = Self { size: 0, align: 1, flags: 0 };

    #[inline(always)]
    pub const fn new(size: bx_u32, align: bx_u16) -> Self {
        Self { size, align, flags: 0 }
    }

    #[inline(always)]
    pub const fn padded_size(&self) -> bx_u32 {
        // size redondeado hacia arriba al múltiplo de align.
        let a = self.align as bx_u32;
        if a == 0 { return self.size; }
        (self.size + a - 1) & !(a - 1)
    }

    #[inline(always)]
    pub const fn is_zst(&self) -> bool { self.size == 0 }

    /// Layout válido si align es potencia de 2 y >= 1.
    #[inline(always)]
    pub const fn is_valid(&self) -> bool {
        self.align >= 1 && (self.align & (self.align - 1)) == 0
    }
}

bitflags::bitflags! {
    /// Flags de `TypeLayout.flags`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct LayoutFlags: bx_u16 {
        /// El tipo NO contiene punteros (puede memcpy-arse libremente).
        const POD            = 1 << 0;
        /// Se puede copiar bit-a-bit pero ojo con drop.
        const TRIVIAL_COPY   = 1 << 1;
        /// El layout es estable across compilers/lenguajes.
        const FFI_STABLE     = 1 << 2;
        /// El tipo es `Send` (transferible entre threads).
        const SEND           = 1 << 3;
        /// El tipo es `Sync` (compartible por &T entre threads).
        const SYNC           = 1 << 4;
        /// El tipo se compone exclusivamente de tipos `repr(C)`.
        const REPR_C         = 1 << 5;
        /// Empaquetado (sin padding interno, equivalente a `#[repr(packed)]`).
        const PACKED         = 1 << 6;
        /// Contiene generación de handle, no marshallar a memoria persistente.
        const VOLATILE       = 1 << 7;
    }
}
