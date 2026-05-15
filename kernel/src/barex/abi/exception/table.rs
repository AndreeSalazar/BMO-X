//! Tabla de unwinding compacta BMO. Reemplaza `.eh_frame` (DWARF).

use crate::barex::abi::primitives::{bx_u32, bx_u64};
use crate::barex::abi::type_system::TypeId;

/// Una entrada por región de código que puede lanzar.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UnwindEntry {
    /// RVA de inicio de la región (función o try-block).
    pub start_rva: bx_u32,
    /// RVA exclusivo de fin.
    pub end_rva: bx_u32,
    /// RVA del handler (landing pad).
    pub handler_rva: bx_u32,
    /// Tipo de excepción capturada (TypeId::VOID = catch-all).
    pub catch_type: TypeId,
}

/// Tabla completa: cabecera mínima + slice contiguo.
#[repr(C)]
pub struct UnwindTable<'a> {
    pub n_entries: bx_u32,
    /// RVA base donde aplica esta tabla (módulo BEF cargado).
    pub module_base: bx_u64,
    pub entries: &'a [UnwindEntry],
}

impl<'a> UnwindTable<'a> {
    pub const EMPTY: Self = Self { n_entries: 0, module_base: 0, entries: &[] };

    pub const fn from_slice(base: bx_u64, entries: &'a [UnwindEntry]) -> Self {
        Self {
            n_entries: entries.len() as bx_u32,
            module_base: base,
            entries,
        }
    }

    /// Busca la entrada cuya región contiene `rva`. O(log n) candidato si
    /// las entradas vienen ordenadas; por ahora O(n) lineal.
    pub fn lookup(&self, rva: bx_u32) -> Option<&UnwindEntry> {
        self.entries.iter().find(|e| rva >= e.start_rva && rva < e.end_rva)
    }
}
