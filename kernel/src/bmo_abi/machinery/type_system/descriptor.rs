//! `TypeDescriptor` — descriptor canónico de un tipo BMO.
//!
//! Cualquier struct/enum/función de cualquier lenguaje se describe con uno.

use crate::bmo_abi::primitives::{bx_u32, bx_u64};
use crate::bmo_abi::string::BmoStr;
use crate::bmo_abi::type_system::kind::TypeKind;
use crate::bmo_abi::type_system::layout::TypeLayout;

/// Identificador estable de un tipo: hash BLAKE3 truncado a 64 bits.
///
/// Sobre la palabra del lenguaje se construye igual sin importar el origen
/// (Rust `TypeId`, C++ `typeid`, .NET `RuntimeTypeHandle` → mismo `TypeId`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(pub bx_u64);

impl TypeId {
    pub const VOID: Self = Self(0);

    #[inline(always)]
    pub const fn from_hash(h: bx_u64) -> Self { Self(h) }

    #[inline(always)]
    pub const fn raw(self) -> bx_u64 { self.0 }
}

/// Descriptor de un campo de struct/union.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FieldDescriptor<'a> {
    pub name: BmoStr<'a>,
    pub type_id: TypeId,
    /// Offset en bytes desde el inicio del struct contenedor.
    pub offset: bx_u32,
    /// Bits de campo de bits (0 si es campo normal).
    pub bit_width: bx_u32,
}

/// Descriptor de una variante de enum (tagged union).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VariantDescriptor<'a> {
    pub name: BmoStr<'a>,
    /// Discriminante (valor del tag en el byte 0..N del enum).
    pub discriminant: bx_u64,
    /// Tipo del payload (TypeId::VOID si la variante no lleva datos).
    pub payload_type: TypeId,
}

/// Descriptor canónico de un tipo BMO.
///
/// `repr(C)` para que C/C++/Java/etc. lo lean directo desde una sección BEF.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TypeDescriptor<'a> {
    pub id: TypeId,
    pub kind: TypeKind,
    /// Padding explícito para alineación.
    pub _pad: [u8; 7],
    pub layout: TypeLayout,
    /// Nombre fully-qualified (`crate::module::Type` o `java/lang/String`).
    pub name: BmoStr<'a>,
    /// TypeId del lenguaje origen (referencia a `lang_bridge::LangDescriptor`).
    pub source_lang: bx_u32,
    /// Versión del descriptor (para evolución de schema).
    pub version: bx_u32,
}

impl<'a> TypeDescriptor<'a> {
    pub const fn new_void() -> Self {
        Self {
            id: TypeId::VOID,
            kind: TypeKind::Void,
            _pad: [0; 7],
            layout: TypeLayout::ZST,
            name: BmoStr::from_str("void"),
            source_lang: 0,
            version: 1,
        }
    }

    #[inline(always)]
    pub const fn is_void(&self) -> bool { self.id.raw() == 0 }
}
