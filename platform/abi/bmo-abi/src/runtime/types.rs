//! `types` -- TypeRegistry: registro de tipos del BMO ABI.
//!
//! Almacena metadatos de hasta 256 tipos (estructuras, enums, uniones,
//! funciones) para que el sistema de reflexion y los bridges de lenguaje
//! puedan inspeccionar firmas y layouts en tiempo de ejecucion.
//!
//! ## Integracion con `bmo_abi::types`
//!
//! TypeMeta es el header fijo de 32 bytes. Para tipos compuestos (structs,
//! funciones), el TypeRegistry puede opcionalmente almacenar:
//! - `TypeField` descriptors (via `bmo_abi::types::TypeField`)
//! - `FunctionSignature` (via `bmo_abi::types::FunctionSignature`)
//! - `ParamDescriptor` lists (via `bmo_abi::types::ParamDescriptor`)

use crate::bmo_abi::primitives::{bx_u32, bx_u64, bx_u8};
use crate::bmo_abi::types::{FieldTable, FunctionSignature, ParamDescriptor};

/// Capacidad maxima del registro de tipos.
pub const TYPE_REGISTRY_CAP: usize = 256;

/// Kind constants for TypeMeta.kind field.
pub mod type_kind {
    use crate::bmo_abi::primitives::bx_u8;
    pub const STRUCT: bx_u8 = 0;
    pub const ENUM: bx_u8 = 1;
    pub const UNION: bx_u8 = 2;
    pub const FUNCTION: bx_u8 = 3;
    pub const PRIMITIVE: bx_u8 = 4;
    pub const POINTER: bx_u8 = 5;
    pub const ARRAY: bx_u8 = 6;
    pub const ALIAS: bx_u8 = 7;
}

/// Metadatos de un tipo registrado.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TypeMeta {
    /// FNV-1a 64-bit hash del nombre del tipo.
    pub name_hash: bx_u64,
    /// Tamano en bytes del tipo.
    pub size: bx_u64,
    /// Alineacion en bytes.
    pub align: bx_u32,
    /// Kind: 0=struct, 1=enum, 2=union, 3=fn, 4=primitive, 5=pointer, 6=array, 7=alias
    pub kind: bx_u8,
    /// Numero de campos (0 para primitivos).
    pub field_count: bx_u32,
}
const _: () = assert!(core::mem::size_of::<TypeMeta>() == 32);

impl TypeMeta {
    pub const fn empty() -> Self {
        Self {
            name_hash: 0,
            size: 0,
            align: 0,
            kind: 0,
            field_count: 0,
        }
    }
}

/// Entry in the type registry: fixed header + optional extended metadata.
#[derive(Debug, Clone, Copy)]
pub struct TypeEntry {
    pub meta: TypeMeta,
    /// Field descriptors (for struct/enum/union, None for primitives/functions).
    pub fields: Option<FieldTable>,
    /// Function signature (only for kind=FUNCTION).
    pub func_sig: Option<FunctionSignature>,
    /// Parameter descriptors (only for kind=FUNCTION).
    pub params: Option<[ParamDescriptor; 16]>,
}

impl TypeEntry {
    pub const fn empty() -> Self {
        Self {
            meta: TypeMeta::empty(),
            fields: None,
            func_sig: None,
            params: None,
        }
    }
}

/// Registro de tipos con capacidad fija (sin heap).
pub struct TypeRegistry {
    entries: [TypeEntry; TYPE_REGISTRY_CAP],
    count: usize,
}

impl TypeRegistry {
    pub const fn new() -> Self {
        Self {
            entries: [TypeEntry::empty(); TYPE_REGISTRY_CAP],
            count: 0,
        }
    }

    /// Register a simple type (primitive, alias, pointer).
    pub fn register(&mut self, meta: TypeMeta) -> Option<bx_u32> {
        if self.count >= TYPE_REGISTRY_CAP {
            return None;
        }
        let idx = self.count as bx_u32;
        self.entries[self.count] = TypeEntry {
            meta,
            fields: None,
            func_sig: None,
            params: None,
        };
        self.count += 1;
        Some(idx)
    }

    /// Register a struct/enum/union type with field descriptors.
    pub fn register_struct(&mut self, meta: TypeMeta, fields: FieldTable) -> Option<bx_u32> {
        if self.count >= TYPE_REGISTRY_CAP {
            return None;
        }
        let idx = self.count as bx_u32;
        self.entries[self.count] = TypeEntry {
            meta,
            fields: Some(fields),
            func_sig: None,
            params: None,
        };
        self.count += 1;
        Some(idx)
    }

    /// Register a function type with signature and parameter descriptors.
    pub fn register_function(
        &mut self,
        meta: TypeMeta,
        sig: FunctionSignature,
        params: [ParamDescriptor; 16],
    ) -> Option<bx_u32> {
        if self.count >= TYPE_REGISTRY_CAP {
            return None;
        }
        let idx = self.count as bx_u32;
        self.entries[self.count] = TypeEntry {
            meta,
            fields: None,
            func_sig: Some(sig),
            params: Some(params),
        };
        self.count += 1;
        Some(idx)
    }

    pub fn lookup(&self, name_hash: bx_u64) -> Option<&TypeEntry> {
        self.entries[..self.count]
            .iter()
            .find(|e| e.meta.name_hash == name_hash)
    }

    pub fn get(&self, idx: bx_u32) -> Option<&TypeEntry> {
        self.entries
            .get(idx as usize)
            .filter(|_| (idx as usize) < self.count)
    }

    /// Look up a type and return its field descriptors (if any).
    pub fn get_fields(&self, type_id: bx_u32) -> Option<&FieldTable> {
        self.get(type_id).and_then(|e| e.fields.as_ref())
    }

    /// Look up a function type and return its signature + params.
    pub fn get_function(
        &self,
        type_id: bx_u32,
    ) -> Option<(&FunctionSignature, &[ParamDescriptor])> {
        self.get(type_id)
            .and_then(|e| match (&e.func_sig, &e.params) {
                (Some(sig), Some(params)) => {
                    let n = sig.param_count as usize;
                    Some((sig, &params[..n.min(16)]))
                }
                _ => None,
            })
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// True if at least one type has been registered.
    pub fn is_valid(&self) -> bool {
        self.count > 0
    }
}
