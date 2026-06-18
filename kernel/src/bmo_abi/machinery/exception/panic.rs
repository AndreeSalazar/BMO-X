//! `BmoPanic` — payload de un panic del BMO ABI.

use crate::bmo_abi::primitives::bx_u32;
use crate::bmo_abi::status::BmoStatus;
use crate::bmo_abi::string::BmoStr;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanicKind {
    /// Aserción fallida (`assert!`, `BMO_ASSERT`).
    Assert       = 0,
    /// Acceso fuera de rango (slice, array).
    OutOfBounds  = 1,
    /// División por cero / overflow aritmético.
    Math         = 2,
    /// Allocator falló y no hay recovery.
    OutOfMemory  = 3,
    /// Handle inválido o use-after-free detectado por generación.
    BadHandle    = 4,
    /// Custom — payload a discreción del lenguaje origen.
    Custom       = 0xFFFF_FFFF,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoPanic<'a> {
    pub kind: PanicKind,
    pub status: BmoStatus,
    /// Mensaje legible humano.
    pub message: BmoStr<'a>,
    /// Archivo:línea de origen (BMO no usa `__FILE__`/`__LINE__`).
    pub file: BmoStr<'a>,
    pub line: bx_u32,
    pub column: bx_u32,
}

impl<'a> BmoPanic<'a> {
    pub const fn new(kind: PanicKind, message: BmoStr<'a>) -> Self {
        Self {
            kind,
            status: BmoStatus { code: 1, flags: 0, value: kind as u64 as u64 },
            message,
            file: BmoStr::EMPTY,
            line: 0,
            column: 0,
        }
    }
}
