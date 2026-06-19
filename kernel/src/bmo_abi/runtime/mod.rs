//! `runtime` — agregador único de todas las sub-registries del BMO ABI.
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

extern crate alloc;

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::primitives::bx_u32;
use crate::bmo_abi::type_system::{TypeRegistry, TypeId, TypeDescriptor};
use crate::bmo_abi::lang_bridge::{LangRegistry, LangDescriptor};
use crate::bmo_abi::vtable::BmoVTable;
use crate::bmo_abi::reflect::ReflectQuery;
use crate::bmo_abi::exception::UnwindTable;

/// Agregador de todas las tablas BMO ABI cargadas para un proceso/módulo.
///
/// `repr(C)` porque puede ser pasado a apps via syscall (su layout debe
/// ser estable).
#[repr(C)]
pub struct BmoRuntime<'a> {
    pub types: TypeRegistry<'a>,
    pub langs: LangRegistry<'a>,
    pub vtables: &'a [BmoVTable<'a>],
    pub unwind: UnwindTable<'a>,
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

/// Valida un runtime cruzando referencias entre tablas.
///
/// Garantía: instanciar un runtime nunca panica. Ante datos malformados
/// devuelve `Err(BxError::InvalidArgument)` o `BxError::NotFound` con
/// detalle del problema en serial.
pub fn validate_runtime(rt: &BmoRuntime<'_>) -> BxResult<()> {
    // ─── Check 1: type registry no debe tener TypeIds duplicados ───
    let n = rt.types.len();
    if n > 1 {
        let descriptors = rt.types.iter();
        let descs: alloc::vec::Vec<&TypeDescriptor> = descriptors.collect();
        for i in 0..(n - 1) {
            for j in (i + 1)..n {
                if descs[i].id == descs[j].id {
                    crate::diag::warn_u64(
                        "bmo_abi::runtime",
                        "duplicate TypeId in registry",
                        descs[i].id.raw(),
                    );
                    return Err(BxError::InvalidArgument);
                }
            }
        }
    }

    // ─── Check 2: cada TypeDescriptor.name debe apuntar a UTF-8 válido ──
    for td in rt.types.iter() {
        if !td.name.is_empty() {
            if core::str::from_utf8(unsafe {
                core::slice::from_raw_parts(td.name.ptr, td.name.len as usize)
            }).is_err() {
                crate::diag::warn(
                    "bmo_abi::runtime",
                    "TypeDescriptor.name is not valid UTF-8",
                );
                return Err(BxError::InvalidArgument);
            }
        }
    }

    // ─── Check 3: vtables deben tener magic BVT1 y counts válidos ──
    for vt in rt.vtables {
        if !vt.is_valid() {
            crate::diag::warn(
                "bmo_abi::runtime",
                "vtable has invalid magic (expected BVT1)",
            );
            return Err(BxError::InvalidArgument);
        }
        if vt.entries.len() != vt.header.n_entries as usize {
            crate::diag::warn(
                "bmo_abi::runtime",
                "vtable entry count mismatch with header",
            );
            return Err(BxError::InvalidArgument);
        }
    }

    // ─── Check 4: lang registry source_lang debe ser ID conocido ──
    for td in rt.types.iter() {
        let lang_id = td.source_lang;
        if lang_id != 0 && rt.langs.lookup(lang_id).is_err() {
            crate::diag::warn_u64(
                "bmo_abi::runtime",
                "TypeDescriptor references unknown language",
                lang_id as u64,
            );
            return Err(BxError::NotFound);
        }
    }

    // ─── Check 5: version debe ser != 0 (reservado) ──
    if rt.version == 0 {
        crate::diag::warn(
            "bmo_abi::runtime",
            "runtime version 0 is reserved",
        );
        return Err(BxError::InvalidArgument);
    }

    Ok(())
}
