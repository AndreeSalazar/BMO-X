//! `BmoHandle` — handle opaco 64-bit con generación.
//!
//! Layout:
//! ```text
//!   bit 63        : tag        (0 = recurso, 1 = canal/cola)
//!   bits 62..56   : kind       (7 bits — 128 tipos)
//!   bits 55..40   : generation (16 bits — invalida UAF)
//!   bits 39..0    : index      (40 bits — 1 trillón de slots)
//! ```

use super::kind::HandleKind;
use crate::bmo_abi::primitives::{bx_u16, bx_u64, bx_u8};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BmoHandle(pub bx_u64);

impl BmoHandle {
    pub const NULL: Self = Self(0);

    /// Handle inválido (distinto de NULL para detectar errores).
    /// Generación = 0xFFFF, index = 0xFFF... — nunca se asigna un handle real.
    pub const INVALID: Self = Self(0x0000_FFFF_FFFF_FFFF);

    #[inline(always)]
    pub const fn new(kind: HandleKind, generation: bx_u16, index: bx_u64) -> Self {
        let tag = (kind.tag() as bx_u64) << 63;
        let kind_bits = ((kind.code() as bx_u64) & 0x7F) << 56;
        let gen_bits = ((generation as bx_u64) & 0xFFFF) << 40;
        let idx_bits = index & 0x000000FF_FFFFFFFF;
        Self(tag | kind_bits | gen_bits | idx_bits)
    }

    #[inline(always)]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub const fn is_resource(self) -> bool {
        (self.0 >> 63) == 0
    }

    #[inline(always)]
    pub const fn is_active(self) -> bool {
        (self.0 >> 63) == 1
    }

    #[inline(always)]
    pub const fn kind_code(self) -> bx_u8 {
        ((self.0 >> 56) & 0x7F) as bx_u8
    }

    /// Decodifica el `HandleKind`. `None` si el código es desconocido.
    #[inline(always)]
    pub const fn kind(self) -> Option<HandleKind> {
        HandleKind::from_code(self.kind_code())
    }

    #[inline(always)]
    pub const fn generation(self) -> bx_u16 {
        ((self.0 >> 40) & 0xFFFF) as bx_u16
    }

    #[inline(always)]
    pub const fn index(self) -> bx_u64 {
        self.0 & 0x000000FF_FFFFFFFF
    }

    /// Verifica que el handle es del kind esperado. Útil para asserts en hot paths.
    #[inline(always)]
    pub fn is_kind(self, expected: HandleKind) -> bool {
        self.kind_code() == expected.code()
    }
}
