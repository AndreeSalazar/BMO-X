//! `BmoRuntime` — punto único de acceso a TODAS las sub-registries del BMO ABI.
//!
//! Cuando el kernel arranca y carga un BEF, instancia un `BmoRuntime` con
//! las secciones nuevas (`TypeMap`, `VTables`, `LangBridge`, `Reflect`,
//! `Closures`). Cualquier consumidor downstream (sched, syscall, shell)
//! pregunta a `runtime.types()`, `runtime.langs()`, etc. en vez de tocar
//! módulos sueltos.
//!
//! Reemplaza el patrón C de "globals dispersos" (`__cxa_*`, `__libc_*`,
//! `KeServiceDescriptorTable`) con un único `BmoRuntime` pasable por
//! handle.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::bx_u32;
use crate::barex::abi::type_system::{TypeRegistry, TypeId, TypeDescriptor};
use crate::barex::abi::lang_bridge::{LangRegistry, LangDescriptor};
use crate::barex::abi::vtable::BmoVTable;
use crate::barex::abi::reflect::ReflectQuery;
use crate::barex::abi::exception::UnwindTable;

/// Agregador de todas las tablas BMO ABI cargadas para un proceso/módulo.
///
/// `repr(C)` porque puede ser pasado a apps via syscall (su layout debe
/// ser estable).
#[repr(C)]
pub struct BmoRuntime<'a> {
    types: TypeRegistry<'a>,
    langs: LangRegistry<'a>,
    vtables: &'a [BmoVTable<'a>],
    unwind: UnwindTable<'a>,
    /// Versión del runtime (incrementa con cambios incompatibles).
    pub version: bx_u32,
}

impl<'a> BmoRuntime<'a> {
    /// Runtime vacío — usado en bootstrap antes de cargar el primer BEF.
    pub const EMPTY: Self = Self {
        types: TypeRegistry::empty(),
        langs: LangRegistry::EMPTY,
        vtables: &[],
        unwind: UnwindTable::EMPTY,
        version: 1,
    };

    pub const fn new(
        types: TypeRegistry<'a>,
        langs: LangRegistry<'a>,
        vtables: &'a [BmoVTable<'a>],
        unwind: UnwindTable<'a>,
    ) -> Self {
        Self { types, langs, vtables, unwind, version: 1 }
    }

    #[inline(always)] pub fn types(&self) -> &TypeRegistry<'a> { &self.types }
    #[inline(always)] pub fn langs(&self) -> &LangRegistry<'a> { &self.langs }
    #[inline(always)] pub fn vtables(&self) -> &[BmoVTable<'a>] { self.vtables }
    #[inline(always)] pub fn unwind(&self) -> &UnwindTable<'a> { &self.unwind }

    /// Helper de reflection (combina type registry + symbols).
    pub fn reflect(&self) -> ReflectQuery<'_> {
        ReflectQuery::new(&self.types)
    }

    /// Busca un tipo por id. Atajo común.
    pub fn type_of(&self, id: TypeId) -> BxResult<&TypeDescriptor<'a>> {
        self.types.lookup(id)
    }

    /// Busca un lenguaje por id. Atajo común.
    pub fn lang_of(&self, id: bx_u32) -> BxResult<&LangDescriptor<'a>> {
        self.langs.lookup(id)
    }

    /// Estadísticas rápidas para debug/shell.
    pub fn stats(&self) -> RuntimeStats {
        RuntimeStats {
            type_count: self.types.len() as bx_u32,
            lang_count: self.langs.count() as bx_u32,
            vtable_count: self.vtables.len() as bx_u32,
            unwind_entries: self.unwind.n_entries,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RuntimeStats {
    pub type_count: bx_u32,
    pub lang_count: bx_u32,
    pub vtable_count: bx_u32,
    pub unwind_entries: bx_u32,
}

/// Garantía: instanciar un runtime nunca panica. Ante datos malformados
/// devuelve `Err(BxError::InvalidArgument)`.
pub fn validate_runtime(_rt: &BmoRuntime<'_>) -> BxResult<()> {
    // Stub — sesiones futuras chequearán cross-references entre tablas.
    Err(BxError::NotImplemented)
}
