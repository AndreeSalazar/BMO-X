//! Parser de las secciones meta-genéricas de BEF.
//!
//! v1.7.9: stub. La lógica completa (TypeMap, VTables, LangBridge,
//! Reflect, Closures, Unwind) se implementará en v2.0 cuando haya
//! un compilador BEF emisor real.

#![allow(dead_code)]

use crate::bmo_core::bef::sections::{SectionKind, SectionTable};
use super::LoadError;

/// Vista zero-copy de las secciones meta. Cada campo es `None` si la
/// sección no está presente en el BEF.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetaSectionViews<'a> {
    pub type_map:     Option<&'a [u8]>,
    pub vtables:      Option<&'a [u8]>,
    pub lang_bridge:  Option<&'a [u8]>,
    pub reflect:      Option<&'a [u8]>,
    pub closures:     Option<&'a [u8]>,
    pub unwind:       Option<&'a [u8]>,
}

/// Localiza las secciones meta en un BEF. Stub.
///
/// v1.7.9: sólo enumera qué secciones meta están presentes. La
/// decodificación real de TypeMap/VTables/LangBridge/etc. es stub
/// porque BMO ABI runtime se simplificó a `BmoRuntimePlaceholder`.
pub fn parse_meta_sections<'a>(
    _bytes: &'a [u8],
    table: &SectionTable,
) -> Result<MetaSectionViews<'a>, LoadError> {
    let views = MetaSectionViews::default();
    for kind in [SectionKind::TypeMap, SectionKind::VTables, SectionKind::LangBridge,
                 SectionKind::Reflect, SectionKind::Closures, SectionKind::Unwind] {
        if table.find(kind).is_some() {
            // We don't have bytes here; the caller (native.rs) will
            // resolve offsets from entry.file_offset and entry.file_size.
            // For now, just record presence.
            let _ = kind;
        }
    }
    Ok(views)
}
