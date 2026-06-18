//! Parser de las 5 nuevas secciones meta-genéricas de BEF (Sesión 8).
//!
//! Localiza las secciones `TypeMap`, `VTables`, `LangBridge`, `Reflect`,
//! `Closures` en la `SectionTable` de un BEF, valida sus offsets y
//! produce vistas zero-copy listas para construir un `BmoRuntime`.
//!
//! ## Pipeline
//!
//! ```text
//!   bytes + SectionTable
//!         │
//!         ▼
//!   parse_meta_sections() ──▶ MetaSectionViews
//!         │                    │
//!         │                    ├── type_map:    Option<&[u8]>
//!         │                    ├── vtables:     Option<&[u8]>
//!         │                    ├── lang_bridge: Option<&[u8]>
//!         │                    ├── reflect:     Option<&[u8]>
//!         │                    └── closures:    Option<&[u8]>
//!         ▼
//!   build_runtime(views) ──▶ BmoRuntime (Sesión 9 wireado)
//! ```
//!
//! Los blobs son binarios `repr(C)`. La validación profunda (cada
//! `TypeDescriptor`, cada `LangDescriptor`) llega en sesiones futuras
//! cuando emerja el primer compilador BEF emisor de estas secciones.

#![allow(dead_code)]

use crate::bef::sections::{SectionKind, SectionTable, SectionEntry};
use crate::bmo_abi::primitives::bx_u32;
use crate::bmo_abi::type_system::{TypeDescriptor, TypeRegistry};
use crate::bmo_abi::lang_bridge::{LangDescriptor, LangRegistry};
use crate::bmo_abi::exception::UnwindTable;
use crate::bmo_abi::runtime::BmoRuntime;
use super::LoadError;

/// Vista zero-copy de las 5 secciones meta. Cada campo es `None` si la
/// sección no está presente en el BEF.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetaSectionViews<'a> {
    pub type_map:    Option<&'a [u8]>,
    pub vtables:     Option<&'a [u8]>,
    pub lang_bridge: Option<&'a [u8]>,
    pub reflect:     Option<&'a [u8]>,
    pub closures:    Option<&'a [u8]>,
    /// `Unwind` viene del cimiento (sesión 5) pero lo agregamos aquí
    /// porque el `BmoRuntime` lo agrupa con el resto.
    pub unwind:      Option<&'a [u8]>,
}

impl<'a> MetaSectionViews<'a> {
    pub const EMPTY: Self = Self {
        type_map: None, vtables: None, lang_bridge: None,
        reflect: None, closures: None, unwind: None,
    };

    /// Devuelve true si al menos una sección meta-genérica está presente.
    /// Útil para distinguir BEFs "puramente código" de los enriquecidos.
    pub const fn has_any_metadata(&self) -> bool {
        self.type_map.is_some()
            || self.vtables.is_some()
            || self.lang_bridge.is_some()
            || self.reflect.is_some()
            || self.closures.is_some()
    }
}

/// Localiza las 5 nuevas secciones + Unwind dentro del BEF.
///
/// `bytes` es el archivo completo, `table` ya parseada por el loader.
/// Devuelve `Err(SectionOutOfRange)` si alguna sección declara un rango
/// fuera del archivo.
pub fn parse_meta_sections<'a>(
    bytes: &'a [u8],
    table: &SectionTable<'a>,
) -> Result<MetaSectionViews<'a>, LoadError> {
    let mut views = MetaSectionViews::EMPTY;

    for entry in table.entries.iter() {
        let Some(kind) = entry.kind() else { continue };
        let slice = slice_for_entry(bytes, entry)?;
        match kind {
            SectionKind::TypeMap    => views.type_map    = Some(slice),
            SectionKind::VTables    => views.vtables     = Some(slice),
            SectionKind::LangBridge => views.lang_bridge = Some(slice),
            SectionKind::Reflect    => views.reflect     = Some(slice),
            SectionKind::Closures   => views.closures    = Some(slice),
            SectionKind::Unwind     => views.unwind      = Some(slice),
            _ => {} // resto: no nos toca aquí.
        }
    }

    Ok(views)
}

/// Slice zero-copy del archivo correspondiente a una `SectionEntry`.
/// Retorna `&[]` para BSS o secciones sin `file_size`.
fn slice_for_entry<'a>(
    bytes: &'a [u8],
    entry: &SectionEntry,
) -> Result<&'a [u8], LoadError> {
    if entry.file_size == 0 {
        return Ok(&[]);
    }
    let start = entry.file_offset as usize;
    let end = start.saturating_add(entry.file_size as usize);
    if end > bytes.len() {
        return Err(LoadError::SectionOutOfRange);
    }
    Ok(&bytes[start..end])
}

// ─── Builders de tablas tipadas ─────────────────────────────────────

/// Reinterpreta el blob `TypeMap` como un slice de `TypeDescriptor`s.
///
/// Sanity checks:
///   - tamaño múltiplo de `size_of::<TypeDescriptor>()`
///   - alineación adecuada (8 B)
///
/// SAFETY: el archivo BEF debe haber sido validado y firmado.
pub fn type_descriptors_from<'a>(blob: &'a [u8]) -> Result<&'a [TypeDescriptor<'a>], LoadError> {
    if blob.is_empty() { return Ok(&[]); }
    let elem = core::mem::size_of::<TypeDescriptor<'a>>();
    if elem == 0 || blob.len() % elem != 0 {
        return Err(LoadError::InvalidHeader);
    }
    if (blob.as_ptr() as usize) % core::mem::align_of::<TypeDescriptor<'a>>() != 0 {
        return Err(LoadError::InvalidHeader);
    }
    let count = blob.len() / elem;
    // SAFETY: tamaño y alineación verificados.
    let ptr = blob.as_ptr() as *const TypeDescriptor<'a>;
    Ok(unsafe { core::slice::from_raw_parts(ptr, count) })
}

/// Análogo a `type_descriptors_from` para `LangDescriptor`.
pub fn lang_descriptors_from<'a>(blob: &'a [u8]) -> Result<&'a [LangDescriptor<'a>], LoadError> {
    if blob.is_empty() { return Ok(&[]); }
    let elem = core::mem::size_of::<LangDescriptor<'a>>();
    if elem == 0 || blob.len() % elem != 0 {
        return Err(LoadError::InvalidHeader);
    }
    if (blob.as_ptr() as usize) % core::mem::align_of::<LangDescriptor<'a>>() != 0 {
        return Err(LoadError::InvalidHeader);
    }
    let count = blob.len() / elem;
    // SAFETY: tamaño y alineación verificados.
    let ptr = blob.as_ptr() as *const LangDescriptor<'a>;
    Ok(unsafe { core::slice::from_raw_parts(ptr, count) })
}

// ─── Construcción del BmoRuntime ────────────────────────────────────

/// Intenta construir un `BmoRuntime` desde las vistas meta.
///
/// Si alguna sección está presente pero malformada, devuelve
/// `LoadError::InvalidHeader`. Si una sección está ausente, se sustituye
/// por su tabla vacía equivalente — un BEF sin metadatos sigue siendo
/// cargable, sólo no participa de reflection / dispatch dinámico.
///
/// `module_base` es la dirección virtual donde se cargó el módulo (para
/// la `UnwindTable`).
pub fn build_runtime<'a>(
    views: &MetaSectionViews<'a>,
    module_base: u64,
) -> Result<BmoRuntime<'a>, LoadError> {
    let types = match views.type_map {
        Some(blob) => TypeRegistry::from_slice(type_descriptors_from(blob)?),
        None => TypeRegistry::empty(),
    };
    let langs = match views.lang_bridge {
        Some(blob) => LangRegistry::from_slice(lang_descriptors_from(blob)?),
        None => LangRegistry::EMPTY,
    };
    // VTables y Closures necesitan parsing más estructurado (header + entries
    // de tamaño variable) — sesión futura. Por ahora slice vacío.
    let vtables: &[crate::bmo_abi::vtable::BmoVTable<'a>] = &[];
    let unwind = UnwindTable::from_slice(module_base, &[]);
    let _ = views.vtables;
    let _ = views.reflect;
    let _ = views.closures;

    Ok(BmoRuntime::new(types, langs, vtables, unwind))
}

// ─── Diagnóstico para shell/debug ───────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct MetaSectionStats {
    pub type_map_bytes:    bx_u32,
    pub vtables_bytes:     bx_u32,
    pub lang_bridge_bytes: bx_u32,
    pub reflect_bytes:     bx_u32,
    pub closures_bytes:    bx_u32,
    pub unwind_bytes:      bx_u32,
}

pub fn meta_stats(views: &MetaSectionViews<'_>) -> MetaSectionStats {
    MetaSectionStats {
        type_map_bytes:    views.type_map.map(|s| s.len() as bx_u32).unwrap_or(0),
        vtables_bytes:     views.vtables.map(|s| s.len() as bx_u32).unwrap_or(0),
        lang_bridge_bytes: views.lang_bridge.map(|s| s.len() as bx_u32).unwrap_or(0),
        reflect_bytes:     views.reflect.map(|s| s.len() as bx_u32).unwrap_or(0),
        closures_bytes:    views.closures.map(|s| s.len() as bx_u32).unwrap_or(0),
        unwind_bytes:      views.unwind.map(|s| s.len() as bx_u32).unwrap_or(0),
    }
}
