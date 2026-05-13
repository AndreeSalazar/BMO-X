//! Tabla de exports BEF.
//!
//! Reemplaza:
//!   - PE: `IMAGE_EXPORT_DIRECTORY` + EAT + ordinal table
//!   - ELF: `.dynsym` con binding GLOBAL/WEAK
//!
//! Modelo BEF: `(symbol_name, hash, virt_addr, size, flags)`. Sin ordinales
//! como tipo principal — pero se permite acceso por índice (que actúa como
//! ordinal estable durante una versión major).

#![allow(dead_code)]

use crate::barex::abi::primitives::{bx_u32, bx_u64};

/// Una entrada del export table — 32 bytes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct ExportEntry {
    /// Offset al string del nombre del símbolo (en sección Exports).
    pub symbol_name_off: bx_u32,
    /// Hash BLAKE3-32 del nombre — acelera la búsqueda.
    pub symbol_hash: bx_u32,
    /// Dirección virtual (relativa al base) del símbolo.
    pub virt_addr: bx_u64,
    /// Tamaño en bytes (función o dato).
    pub size: bx_u64,
    /// Flags `ExportFlags`.
    pub flags: bx_u32,
    /// Reservado.
    pub _reserved: bx_u32,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ExportFlags: bx_u32 {
        /// Es una función (default; sin esto es un símbolo de dato).
        const FUNCTION       = 1 << 0;
        /// Es weak (otra lib puede sobrescribir).
        const WEAK           = 1 << 1;
        /// Solo visible dentro del proceso (no para hot-reload externo).
        const PROCESS_LOCAL  = 1 << 2;
        /// Marca que requiere capability específica para llamar.
        const NEEDS_CAP      = 1 << 3;
    }
}

pub struct ExportTable<'a> {
    pub entries: &'a [ExportEntry],
    pub strings: &'a [u8],
}

impl<'a> ExportTable<'a> {
    pub fn parse(section_bytes: &'a [u8], entry_count: u32) -> Result<Self, &'static str> {
        let needed = entry_count as usize * core::mem::size_of::<ExportEntry>();
        if section_bytes.len() < needed {
            return Err("export table demasiado pequeña");
        }
        let ptr = section_bytes.as_ptr() as *const ExportEntry;
        let entries = unsafe { core::slice::from_raw_parts(ptr, entry_count as usize) };
        let strings = &section_bytes[needed..];
        Ok(Self { entries, strings })
    }

    /// Búsqueda por hash (rápida — O(n) pero comparando solo u32).
    /// Si hay colisión, fallback a comparar el nombre completo.
    pub fn find_by_name(&self, name: &str) -> Option<&ExportEntry> {
        let target_hash = blake3_hash32(name.as_bytes());
        for e in self.entries {
            if e.symbol_hash == target_hash {
                if self.symbol_name(e).map(|n| n == name).unwrap_or(false) {
                    return Some(e);
                }
            }
        }
        None
    }

    pub fn symbol_name(&self, e: &ExportEntry) -> Option<&'a str> {
        let off = e.symbol_name_off as usize;
        if off + 2 > self.strings.len() { return None; }
        let len = u16::from_le_bytes(self.strings[off..off + 2].try_into().ok()?) as usize;
        let s = &self.strings[off + 2..off + 2 + len];
        core::str::from_utf8(s).ok()
    }
}

/// Hash BLAKE3 truncado a 32 bits — placeholder; la implementación real vive
/// en `signing::blake3_32`. Por ahora un FNV-1a estable como semilla.
pub(crate) fn blake3_hash32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}
