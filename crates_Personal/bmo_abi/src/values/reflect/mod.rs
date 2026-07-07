//! `reflect` — reflexión sobre tipos BMO y BEF cargados.
//!
//! Permite a lenguajes dinámicos (Python, JS, Lua) inspeccionar
//! estructuras, enumeraciones y firmas de función en tiempo de ejecución.
//!
//! ## Nota
//! Este módulo es un esqueleto funcional. El registro completo de tipos
//! se integra con `runtime::TypeRegistry`.

use crate::bmo_abi::primitives::{bx_u8, bx_u32, bx_u64};
use crate::bmo_abi::fundamentals::string::BmoStr;
use crate::bmo_abi::runtime::types::TypeRegistry;

/// Kind de tipo reflectable.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Struct,
    Enum,
    Union,
    Function,
    Primitive,
}

impl TypeKind {
    pub const fn from_u8(v: bx_u8) -> Self {
        match v {
            0 => Self::Struct,
            1 => Self::Enum,
            2 => Self::Union,
            3 => Self::Function,
            _ => Self::Primitive,
        }
    }
}

/// Un tipo registrado en el sistema de reflexión.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoTypeInfo {
    pub name: BmoStr,
    pub kind: TypeKind,
    pub size: bx_u64,
    pub field_count: bx_u32,
}
const _: () = assert!(core::mem::size_of::<BmoTypeInfo>() == 40);

/// Query de reflexión sobre un BEF cargado.
pub struct ReflectQuery;

impl ReflectQuery {
    /// Resolve a type by FNV-1a hash of its name, using a TypeRegistry.
    pub fn resolve_type(registry: &TypeRegistry, name_hash: bx_u64) -> Option<BmoTypeInfo> {
        let entry = registry.lookup(name_hash)?;
        Some(BmoTypeInfo {
            name: unsafe { BmoStr::from_raw(core::ptr::null(), 0) },
            kind: TypeKind::from_u8(entry.meta.kind),
            size: entry.meta.size,
            field_count: entry.meta.field_count,
        })
    }
}
