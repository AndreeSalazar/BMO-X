//! BEF Linker Resolver — resuelve imports contra el Registry.
//!
//! Estrategias:
//! - **Eager binding**: patchea `binding_offset` con la dirección resuelta.
//! - **Lazy binding**: escribe un trampoline que se resuelve al primer llamado
//!   (no implementado en Fase 1; requiere soporte del loader para secciones RX).

#![allow(dead_code)]

use super::registry::Registry;
use crate::bmo_abi::bef::imports::{ImportFlags, ImportTable};

/// Resultados de la resolución de imports.
#[derive(Debug, Clone, Copy)]
pub struct ResolveResult {
    pub total: u32,
    pub resolved: u32,
    pub unresolved: u32,
    pub weak: u32,
}

/// Resuelve todos los imports de una tabla contra el Registry global.
///
/// `binding_data` es un slice mutable de la sección donde se escriben las
/// direcciones resueltas (`.code` o `.data`). `binding_base` es la VA
/// base de esa sección (para convertir `binding_offset` a índice local).
pub fn resolve_imports(
    import_table: &ImportTable,
    binding_data: &mut [u8],
    binding_base: u64,
) -> ResolveResult {
    let mut total = 0u32;
    let mut resolved = 0u32;
    let mut unresolved = 0u32;
    let mut weak = 0u32;

    for entry in import_table.entries {
        total += 1;
        let lib = import_table.library_name(entry).unwrap_or("");
        let sym = import_table.symbol_name(entry).unwrap_or("");

        if sym.is_empty() {
            unresolved += 1;
            continue;
        }

        let addr = Registry::lookup(lib, sym);

        if addr != 0 {
            patch_binding_offset(entry.binding_offset, addr, binding_data, binding_base);
            resolved += 1;
        } else if entry.flags & ImportFlags::WEAK.bits() != 0 {
            patch_binding_offset(entry.binding_offset, 0, binding_data, binding_base);
            weak += 1;
        } else {
            unresolved += 1;
        }
    }

    ResolveResult {
        total,
        resolved,
        unresolved,
        weak,
    }
}

/// Patchea `addr` (8 bytes LE) en `binding_data` en el offset correcto.
fn patch_binding_offset(
    binding_offset: u64,
    addr: u64,
    binding_data: &mut [u8],
    binding_base: u64,
) {
    if binding_offset == 0 {
        return;
    }
    let rel = binding_offset.saturating_sub(binding_base);
    let start = rel as usize;
    if start + 8 <= binding_data.len() {
        binding_data[start..start + 8].copy_from_slice(&addr.to_le_bytes());
    }
}
