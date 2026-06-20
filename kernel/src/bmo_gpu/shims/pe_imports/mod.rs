//! Parser del Import Directory de PE/COFF.
//!
//! El Import Directory está en `OptionalHeader.DataDirectory[1]`. Es un
//! array de `IMAGE_IMPORT_DESCRIPTOR` terminado por uno con todos campos
//! a cero. Cada descriptor referencia una DLL y dos arrays paralelos:
//!   - **OriginalFirstThunk** (INT) — Import Name Table, leído por el devour.
//!   - **FirstThunk** (IAT) — donde el loader escribe las direcciones reales.

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64};

/// Una entrada del Import Directory — 20 bytes.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageImportDescriptor {
    /// RVA al INT (Import Name Table). Si es 0, fin del array.
    pub original_first_thunk: bx_u32,
    /// Timestamp de bind (ignorado en la mayoría de PEs).
    pub time_date_stamp: bx_u32,
    /// Forwarder chain (raro).
    pub forwarder_chain: bx_u32,
    /// RVA al nombre de la DLL (ASCII, '\0'-terminated).
    pub name_rva: bx_u32,
    /// RVA al IAT (Import Address Table) — donde escribimos las direcciones.
    pub first_thunk_iat: bx_u32,
}

impl ImageImportDescriptor {
    pub const SIZE: usize = 20;

    pub fn is_terminator(&self) -> bool {
        let oft = self.original_first_thunk;
        let n = self.name_rva;
        let ft = self.first_thunk_iat;
        oft == 0 && n == 0 && ft == 0
    }
}

/// Una entrada del INT/IAT — 8 bytes (PE32+).
///
/// Si bit 63 = 1: import por ordinal (bits 15..0 = ordinal).
/// Si bit 63 = 0: bits 30..0 = RVA al `IMAGE_IMPORT_BY_NAME`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct ImageThunk(pub bx_u64);

impl ImageThunk {
    pub fn is_ordinal(&self) -> bool {
        (self.0 & (1u64 << 63)) != 0
    }
    pub fn ordinal(&self) -> Option<bx_u16> {
        if self.is_ordinal() { Some((self.0 & 0xFFFF) as u16) } else { None }
    }
    pub fn name_rva(&self) -> Option<bx_u32> {
        if !self.is_ordinal() && self.0 != 0 { Some((self.0 & 0x7FFF_FFFF) as u32) } else { None }
    }
    pub fn is_terminator(&self) -> bool { self.0 == 0 }
}

/// `IMAGE_IMPORT_BY_NAME` — el `Name` está después de `Hint`.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImageImportByName {
    pub hint: bx_u16,
    // luego: nombre ASCII '\0'-terminated.
}

/// Una import resuelta lista para escribir en el IAT del PE.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedImport {
    /// Offset (RVA) en el IAT donde escribir la dirección.
    pub iat_rva: bx_u32,
    /// Dirección virtual final del símbolo dentro de FastOS.
    pub virt_addr: bx_u64,
    /// Backend al que se resolvió (informativo).
    pub backend: crate::bmo_gpu::shims::pe_thunks::ThunkTarget,
}

/// Convierte un RVA a offset dentro del archivo, dado el conjunto de
/// section headers PE.
pub fn rva_to_file_offset(
    rva: bx_u32,
    sections: &[crate::bmo_core::bef::loader::pe::PeSectionHeader],
) -> Option<usize> {
    for s in sections {
        let va = s.virtual_address;
        let vsz = s.virtual_size;
        if rva >= va && rva < va.saturating_add(vsz) {
            let delta = rva - va;
            return Some((s.pointer_to_raw_data + delta) as usize);
        }
    }
    None
}

/// Lee un nombre ASCII '\0'-terminated del archivo PE en `offset`.
pub fn read_cstr<'a>(bytes: &'a [u8], offset: usize, max: usize) -> Option<&'a str> {
    if offset >= bytes.len() { return None; }
    let end_max = core::cmp::min(offset + max, bytes.len());
    let slice = &bytes[offset..end_max];
    let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    core::str::from_utf8(&slice[..len]).ok()
}
