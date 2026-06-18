//! `LangDescriptor` — ficha técnica de un lenguaje cliente del BMO ABI.

use crate::bmo_abi::primitives::{bx_u16, bx_u32};
use crate::bmo_abi::string::BmoStr;
use crate::bmo_abi::lang_bridge::features::LangFeatures;
use crate::bmo_abi::lang_bridge::ids::LANG_UNKNOWN;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LangVersion {
    pub major: bx_u16,
    pub minor: bx_u16,
    pub patch: bx_u32,
}

impl LangVersion {
    pub const ZERO: Self = Self { major: 0, minor: 0, patch: 0 };
    pub const fn new(major: u16, minor: u16, patch: u32) -> Self {
        Self { major: major as bx_u16, minor: minor as bx_u16, patch }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LangDescriptor<'a> {
    /// ID estable (ver `ids`).
    pub id: bx_u32,
    /// Nombre humano: "Rust", "Java", "Python".
    pub name: BmoStr<'a>,
    /// Versión del compilador / runtime esperada.
    pub version: LangVersion,
    /// Features del lenguaje.
    pub features: LangFeatures,
    /// Hash del esquema de mangling (para que un consumer sepa cómo leer
    /// nombres mangled de este lenguaje).
    pub mangling_hash: bx_u32,
    /// Reservado para extensión futura sin romper layout.
    pub _reserved: [bx_u32; 6],
}

impl<'a> LangDescriptor<'a> {
    pub const UNKNOWN: Self = Self {
        id: LANG_UNKNOWN,
        name: BmoStr::EMPTY,
        version: LangVersion::ZERO,
        features: LangFeatures::empty(),
        mangling_hash: 0,
        _reserved: [0; 6],
    };

    pub const fn new(
        id: bx_u32,
        name: BmoStr<'a>,
        version: LangVersion,
        features: LangFeatures,
    ) -> Self {
        Self {
            id, name, version, features,
            mangling_hash: 0,
            _reserved: [0; 6],
        }
    }
}
