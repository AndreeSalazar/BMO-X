//! Tabla de símbolos BEF — para debug + dynamic linking + backtraces.
//!
//! Reemplaza `.symtab`/`.dynsym` (ELF) y `IMAGE_SYMBOL` (PE/COFF). Una sola
//! tabla con visibilidad y binding explícitos.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u8, bx_u32, bx_u64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolKind {
    /// Marcador (no apunta a nada).
    NoType   = 0x00,
    /// Función.
    Function = 0x01,
    /// Dato (variable global, constante).
    Object   = 0x02,
    /// Sección entera (símbolo sintetizado).
    Section  = 0x03,
    /// Archivo de origen (debug).
    File     = 0x04,
    /// TLS slot.
    Tls      = 0x05,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolBinding {
    /// Local — solo visible dentro del módulo.
    Local    = 0x00,
    /// Global — visible para linking dinámico.
    Global   = 0x01,
    /// Weak — global pero puede ser sobrescrito.
    Weak     = 0x02,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolVisibility {
    /// Default — visible según binding.
    Default  = 0x00,
    /// Hidden — global pero no exportable a otros módulos.
    Hidden   = 0x01,
    /// Internal — solo el linker lo ve.
    Internal = 0x02,
    /// Protected — visible global pero no preemptable.
    Protected = 0x03,
}

/// Una entrada del symbol table — 32 bytes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct Symbol {
    /// Offset al string del nombre (en sección Symbols).
    pub name_off: bx_u32,
    /// Hash BLAKE3-32 del nombre.
    pub name_hash: bx_u32,
    /// Dirección virtual relativa al base.
    pub virt_addr: bx_u64,
    /// Tamaño en bytes.
    pub size: bx_u64,
    /// `SymbolKind`.
    pub kind: bx_u8,
    /// `SymbolBinding`.
    pub binding: bx_u8,
    /// `SymbolVisibility`.
    pub visibility: bx_u8,
    /// Índice de sección donde vive (0xFF = ABS, 0xFE = COMMON).
    pub section_idx: bx_u8,
    /// Reservado.
    pub _reserved: bx_u32,
}

impl Symbol {
    pub const SIZE: usize = 32;

    pub fn kind(&self) -> Option<SymbolKind> {
        match self.kind {
            0x00 => Some(SymbolKind::NoType),
            0x01 => Some(SymbolKind::Function),
            0x02 => Some(SymbolKind::Object),
            0x03 => Some(SymbolKind::Section),
            0x04 => Some(SymbolKind::File),
            0x05 => Some(SymbolKind::Tls),
            _ => None,
        }
    }

    pub fn binding(&self) -> Option<SymbolBinding> {
        match self.binding {
            0x00 => Some(SymbolBinding::Local),
            0x01 => Some(SymbolBinding::Global),
            0x02 => Some(SymbolBinding::Weak),
            _ => None,
        }
    }
}

/// Vista del symbol table.
pub struct SymbolTable<'a> {
    pub entries: &'a [Symbol],
    pub strings: &'a [u8],
}

impl<'a> SymbolTable<'a> {
    pub fn parse(section_bytes: &'a [u8], entry_count: u32) -> Result<Self, &'static str> {
        let needed = entry_count as usize * Symbol::SIZE;
        if section_bytes.len() < needed {
            return Err("symbol table demasiado pequeña");
        }
        let raw_ptr = section_bytes.as_ptr();
        if (raw_ptr as usize) % core::mem::align_of::<Symbol>() != 0 {
            return Err("symbol table pointer mal alineado");
        }
        let ptr = raw_ptr as *const Symbol;
        let entries = unsafe { core::slice::from_raw_parts(ptr, entry_count as usize) };
        let strings = &section_bytes[needed..];
        Ok(Self { entries, strings })
    }

    pub fn name_of(&self, sym: &Symbol) -> Option<&'a str> {
        let off = sym.name_off as usize;
        if off + 2 > self.strings.len() { return None; }
        let len = u16::from_le_bytes(self.strings[off..off + 2].try_into().ok()?) as usize;
        let s = &self.strings[off + 2..off + 2 + len];
        core::str::from_utf8(s).ok()
    }
}
