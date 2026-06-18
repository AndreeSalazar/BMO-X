//! API de queries reflectivas. Stubs — completos en sesiones futuras.

use crate::bmo_abi::string::BmoStr;
use crate::bmo_abi::type_system::{TypeDescriptor, TypeId, TypeRegistry};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectError {
    NoSuchType   = 1,
    NoSuchField  = 2,
    NoSuchMethod = 3,
    NoMetadata   = 4,
}

pub struct ReflectQuery<'a> {
    registry: &'a TypeRegistry<'a>,
}

impl<'a> ReflectQuery<'a> {
    pub const fn new(registry: &'a TypeRegistry<'a>) -> Self {
        Self { registry }
    }

    /// Busca un tipo por nombre fully-qualified.
    pub fn type_by_name(&self, name: BmoStr<'_>) -> Result<&'a TypeDescriptor<'a>, ReflectError> {
        for d in self.registry.iter() {
            if d.name.eq_str(&name) { return Ok(d); }
        }
        Err(ReflectError::NoSuchType)
    }

    /// Búsqueda directa por TypeId.
    pub fn type_by_id(&self, id: TypeId) -> Result<&'a TypeDescriptor<'a>, ReflectError> {
        self.registry.lookup(id).map_err(|_| ReflectError::NoSuchType)
    }
}
