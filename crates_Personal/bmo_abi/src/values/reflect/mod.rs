//! `reflect` — reflexión sobre tipos BMO y BEF cargados.
//!
//! Permite a lenguajes dinámicos (Python, JS, Lua) inspeccionar
//! estructuras, enumeraciones y firmas de función en tiempo de ejecución.
//!
//! ## Nota
//! Este módulo es un esqueleto funcional. El registro completo de tipos
//! se integra con `runtime::TypeRegistry`.

use crate::bmo_abi::primitives::{bx_u32, bx_u64};
use crate::bmo_abi::fundamentals::string::BmoStr;

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

/// Un tipo registrado en el sistema de reflexión.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoTypeInfo {
    pub name: BmoStr,
    pub kind: TypeKind,
    pub size: bx_u64,
    pub field_count: bx_u32,
}

/// Query de reflexión sobre un BEF cargado.
pub struct ReflectQuery;

impl ReflectQuery {
    /// Resolve a type by FNV-1a hash of its name.
    pub fn resolve_type(_name_hash: bx_u64) -> Option<BmoTypeInfo> {
        // TODO: hook into runtime::TypeRegistry when a BEF is loaded
        None
    }
}
