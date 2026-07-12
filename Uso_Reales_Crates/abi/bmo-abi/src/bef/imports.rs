//! Tabla de imports BEF.
//!
//! Reemplaza:
//!   - PE: `IMAGE_IMPORT_DESCRIPTOR` + IAT/HNT (Hint Name Table)
//!   - ELF: `.dynsym` + `.dynstr` + `.rela.plt` + GOT
//!
//! Modelo BEF: cada import es `(library_name, symbol_name, hint)` resuelto
//! a `BmoHandle` en tiempo de carga (eager) o al primer uso (lazy via
//! trampoline en `.code`).

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Una entrada del import table — 24 bytes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct ImportEntry {
    /// Offset (dentro de la sección Imports) al string del nombre de la lib.
    pub library_name_off: bx_u32,
    /// Offset al string del nombre del símbolo.
    pub symbol_name_off: bx_u32,
    /// Hash BLAKE3-32 del símbolo (acelera la búsqueda).
    pub symbol_hash: bx_u32,
    /// Flags `ImportFlags`.
    pub flags: bx_u32,
    /// Offset en `.code` o `.data` donde escribir la dirección resuelta.
    pub binding_offset: bx_u64,
}
const _: () = assert!(core::mem::size_of::<ImportEntry>() == 24);

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ImportFlags: bx_u32 {
        /// Resolver al cargar el binario (vs. al primer uso).
        const EAGER          = 1 << 0;
        /// Es un import opcional (no fallar si no se encuentra).
        const WEAK           = 1 << 1;
        /// El target es un dato (vs. una función).
        const DATA           = 1 << 2;
        /// Lib o símbolo originalmente de Win32 (devour PE).
        const FROM_PE        = 1 << 8;
        /// Lib o símbolo originalmente de glibc/musl (devour ELF).
        const FROM_ELF       = 1 << 9;
    }
}

pub struct ImportTable<'a> {
    pub entries: &'a [ImportEntry],
    pub strings: &'a [u8],
}

impl<'a> ImportTable<'a> {
    pub fn parse(section_bytes: &'a [u8], entry_count: u32) -> Result<Self, &'static str> {
        let needed = entry_count as usize * core::mem::size_of::<ImportEntry>();
        if section_bytes.len() < needed {
            return Err("import table demasiado pequeña");
        }
        let raw_ptr = section_bytes.as_ptr();
        if (raw_ptr as usize) % core::mem::align_of::<ImportEntry>() != 0 {
            return Err("import table pointer mal alineado");
        }
        let ptr = raw_ptr as *const ImportEntry;
        let entries = unsafe { core::slice::from_raw_parts(ptr, entry_count as usize) };
        let strings = &section_bytes[needed..];
        Ok(Self { entries, strings })
    }

    pub fn library_name(&self, e: &ImportEntry) -> Option<&'a str> {
        self.read_str(e.library_name_off)
    }

    pub fn symbol_name(&self, e: &ImportEntry) -> Option<&'a str> {
        self.read_str(e.symbol_name_off)
    }

    fn read_str(&self, off: u32) -> Option<&'a str> {
        let off = off as usize;
        if off >= self.strings.len() { return None; }
        // BEF strings: prefijo de longitud u16 LE + bytes UTF-8 (sin '\0' final).
        let len = u16::from_le_bytes(self.strings.get(off..off + 2)?.try_into().ok()?) as usize;
        let start = off + 2;
        let end = start + len;
        if end > self.strings.len() { return None; }
        core::str::from_utf8(&self.strings[start..end]).ok()
    }
}
