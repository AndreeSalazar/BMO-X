//! `lang::common::types` — Type system compartido entre frontends.
//!
//! Los frontends (BMO, C, Java-BMO, ...) convierten sus tipos a este
//! sistema **canónico** (BMO IR type system). El backend (AOT x86-64)
//! opera solo sobre estos tipos, no sobre tipos específicos del lenguaje.

#![allow(dead_code)]

use core::fmt;

/// Tipo canónico del BMO IR. Es el "denominador común" entre lenguajes.
///
/// Reglas:
/// - `Void` solo se usa como tipo de retorno.
/// - `Bool` se almacena como `i8` (0 o 1).
/// - Enteros con signo: `i8`, `i16`, `i32`, `i64`.
/// - Enteros sin signo: `u8`, `u16`, `u32`, `u64`.
/// - `usize`/`isize` se mapean a `u64`/`i64` en x86-64.
/// - Floats: `f32`, `f64`.
/// - `Ptr` es puntero raw (8 bytes en x86-64).
/// - `Array` tiene tamaño fijo conocido en compilación.
/// - `Struct`/`Union` se resuelven en sema (se vuelven `Ptr` o Aggregate).
/// - `Func` se usa para punteros a función.
/// - `Named` referencia un tipo declarado en el módulo (typedef, struct).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IrType {
    /// Tipo vacío (solo válido como retorno de función).
    Void,
    /// Booleano (1 byte).
    Bool,
    /// Enteros con signo.
    I8, I16, I32, I64,
    /// Enteros sin signo.
    U8, U16, U32, U64,
    /// Punto flotante IEEE 754.
    F32, F64,
    /// Puntero raw (8 bytes en x86-64).
    Ptr,
    /// Array de tamaño fijo.
    Array { elem: IrTypeId, len: u64 },
    /// Puntero a función.
    Func { ret: IrTypeId, params: IrTypeIdList },
    /// Tipo nombrado (definido por el usuario).
    Named(NamedTypeId),
}

/// Identificador opaco de tipo. Los tipos se almacenan en una tabla
/// dentro del módulo/scope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IrTypeId(pub u32);

/// Identificador de un tipo con nombre (typedef, struct, union, enum).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NamedTypeId(pub u32);

/// Lista de tipos (para params de Func).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct IrTypeIdList(pub u32, pub u32); // (offset, count) en una tabla

impl IrType {
    /// `true` si el tipo es un entero (con o sin signo).
    pub fn is_int(self) -> bool {
        matches!(self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 |
            Self::U8 | Self::U16 | Self::U32 | Self::U64
        )
    }
    /// `true` si el tipo es un float.
    pub fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
    /// `true` si el tipo es unsigned.
    pub fn is_unsigned(self) -> bool {
        matches!(self, Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::Bool)
    }
    /// Tamaño en bytes (conocido en compilación).
    pub fn size_bytes(self) -> Option<u8> {
        Some(match self {
            Self::Void => 0,
            Self::Bool | Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 | Self::Ptr => 8,
            Self::Array { .. } | Self::Func { .. } | Self::Named(_) => return None,
        })
    }
    /// Alineación en bytes.
    pub fn align(self) -> u8 {
        self.size_bytes().unwrap_or(8)
    }
}

impl fmt::Display for IrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Void => f.write_str("void"),
            Self::Bool => f.write_str("bool"),
            Self::I8 => f.write_str("i8"),
            Self::I16 => f.write_str("i16"),
            Self::I32 => f.write_str("i32"),
            Self::I64 => f.write_str("i64"),
            Self::U8 => f.write_str("u8"),
            Self::U16 => f.write_str("u16"),
            Self::U32 => f.write_str("u32"),
            Self::U64 => f.write_str("u64"),
            Self::F32 => f.write_str("f32"),
            Self::F64 => f.write_str("f64"),
            Self::Ptr => f.write_str("ptr"),
            Self::Array { elem, len } => write!(f, "[{}; {}]", elem.0, len),
            Self::Func { ret, params } => write!(f, "fn({}) -> {}", params.1, ret.0),
            Self::Named(NamedTypeId(id)) => write!(f, "T{}", id),
        }
    }
}
