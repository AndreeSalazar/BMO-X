//! `TypeRegistry` — registro global runtime de descriptores de tipo.
//!
//! Carga `SectionKind::TypeMap` de cada BEF cargado y permite búsqueda por
//! `TypeId`. Cualquier lenguaje puede preguntar "¿qué es este TypeId?".

use crate::bmo_core::barex::{BxError, BxResult};
use crate::bmo_core::bmo_abi::type_system::descriptor::{TypeDescriptor, TypeId};

/// Tabla de descriptores embebida en una sección BEF.
///
/// Stub — la implementación real cargará desde `SectionKind::TypeMap`.
pub struct TypeRegistry<'a> {
    descriptors: &'a [TypeDescriptor<'a>],
}

impl<'a> TypeRegistry<'a> {
    pub const fn empty() -> Self {
        Self { descriptors: &[] }
    }

    pub const fn from_slice(descriptors: &'a [TypeDescriptor<'a>]) -> Self {
        Self { descriptors }
    }

    /// Busca un descriptor por su `TypeId`. O(n) lineal por ahora; el
    /// loader BEF construirá un índice hash en sesiones futuras.
    pub fn lookup(&self, id: TypeId) -> BxResult<&TypeDescriptor<'a>> {
        for d in self.descriptors.iter() {
            if d.id == id { return Ok(d); }
        }
        Err(BxError::NotFound)
    }

    /// Itera todos los tipos registrados (para reflection completa).
    pub fn iter(&self) -> core::slice::Iter<'_, TypeDescriptor<'a>> {
        self.descriptors.iter()
    }

    pub const fn len(&self) -> usize { self.descriptors.len() }
    pub const fn is_empty(&self) -> bool { self.descriptors.is_empty() }
}
