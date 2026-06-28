//! `types` — TypeRegistry: registro de tipos del BMO ABI.
//!
//! Almacena metadatos de hasta 256 tipos (estructuras, enums, uniones,
//! funciones) para que el sistema de reflexión y los bridges de lenguaje
//! puedan inspeccionar firmas y layouts en tiempo de ejecución.

use crate::bmo_abi::primitives::{bx_u32, bx_u64, bx_u8};

/// Capacidad máxima del registro de tipos.
pub const TYPE_REGISTRY_CAP: usize = 256;

/// Metadatos de un tipo registrado.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TypeMeta {
    /// FNV-1a 64-bit hash del nombre del tipo.
    pub name_hash: bx_u64,
    /// Tamaño en bytes del tipo.
    pub size: bx_u64,
    /// Alineación en bytes.
    pub align: bx_u32,
    /// Kind: 0=struct, 1=enum, 2=union, 3=fn, 4=primitive
    pub kind: bx_u8,
    /// Número de campos (0 para primitivos).
    pub field_count: bx_u32,
}

impl TypeMeta {
    pub const fn empty() -> Self {
        Self { name_hash: 0, size: 0, align: 0, kind: 0, field_count: 0 }
    }
}

/// Registro de tipos con capacidad fija (sin heap).
pub struct TypeRegistry {
    entries: [TypeMeta; TYPE_REGISTRY_CAP],
    count: usize,
}

impl TypeRegistry {
    pub const fn new() -> Self {
        Self {
            entries: [TypeMeta::empty(); TYPE_REGISTRY_CAP],
            count: 0,
        }
    }

    pub fn register(&mut self, meta: TypeMeta) -> Option<bx_u32> {
        if self.count >= TYPE_REGISTRY_CAP {
            return None;
        }
        let idx = self.count as bx_u32;
        self.entries[self.count] = meta;
        self.count += 1;
        Some(idx)
    }

    pub fn lookup(&self, name_hash: bx_u64) -> Option<&TypeMeta> {
        self.entries[..self.count].iter().find(|e| e.name_hash == name_hash)
    }

    pub fn get(&self, idx: bx_u32) -> Option<&TypeMeta> {
        self.entries.get(idx as usize).filter(|_| (idx as usize) < self.count)
    }

    pub fn count(&self) -> usize { self.count }

    /// True if at least one type has been registered.
    pub fn is_valid(&self) -> bool { self.count > 0 }
}
