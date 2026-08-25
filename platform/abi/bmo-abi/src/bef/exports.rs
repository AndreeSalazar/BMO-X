//! Tabla de exports BEF.
//!
//! Reemplaza:
//!   - PE: `IMAGE_EXPORT_DIRECTORY` + EAT + ordinal table
//!   - ELF: `.dynsym` con binding GLOBAL/WEAK
//!
//! Modelo BEF: `(symbol_name, hash, virt_addr, size, flags)`. Sin ordinales
//! como tipo principal -- pero se permite acceso por indice (que actua como
//! ordinal estable durante una version major).

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Una entrada del export table -- 32 bytes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct ExportEntry {
    /// Offset al string del nombre del simbolo (en seccion Exports).
    pub symbol_name_off: bx_u32,
    /// Hash BLAKE3-32 del nombre -- acelera la busqueda.
    pub symbol_hash: bx_u32,
    /// Direccion virtual (relativa al base) del simbolo.
    pub virt_addr: bx_u64,
    /// Tamano en bytes (funcion o dato).
    pub size: bx_u64,
    /// Flags `ExportFlags`.
    pub flags: bx_u32,
    /// Reservado.
    pub _reserved: bx_u32,
}
const _: () = assert!(core::mem::size_of::<ExportEntry>() == 32);

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ExportFlags: bx_u32 {
        /// Es una funcion (default; sin esto es un simbolo de dato).
        const FUNCTION       = 1 << 0;
        /// Es weak (otra lib puede sobrescribir).
        const WEAK           = 1 << 1;
        /// Solo visible dentro del proceso (no para hot-reload externo).
        const PROCESS_LOCAL  = 1 << 2;
        /// Marca que requiere capability especifica para llamar.
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
            return Err("export table demasiado pequena");
        }
        // *** LA ALINEACION, QUE AQUI FALTABA (auditoria 2026-08-24).
        //
        // `imports.rs`, `symbols.rs` y `sections.rs` la comprueban; este era el
        // unico de los cuatro que no. Y no es formalismo: `from_raw_parts`
        // **exige** que el puntero este alineado al tipo, y un `.bex` decide
        // donde empieza su seccion. En x86 una lectura desalineada funciona por
        // accidente, asi que el fallo no se ve nunca aqui -- se ve el dia que
        // esto corra en otro sitio, o cuando el compilador use una instruccion
        // que si lo exija.
        //
        // ** Cuatro ficheros que hacen lo mismo y uno que se olvido una linea es
        // la forma clasica: nadie los lee juntos. Por eso la auditoria los miro
        // en fila.
        let raw_ptr = section_bytes.as_ptr();
        if (raw_ptr as usize) % core::mem::align_of::<ExportEntry>() != 0 {
            return Err("export table pointer mal alineado");
        }
        let ptr = raw_ptr as *const ExportEntry;
        let entries = unsafe { core::slice::from_raw_parts(ptr, entry_count as usize) };
        let strings = &section_bytes[needed..];
        Ok(Self { entries, strings })
    }

    /// Busqueda por hash (rapida -- O(n) pero comparando solo u32).
    /// Si hay colision, fallback a comparar el nombre completo.
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
        if off + 2 > self.strings.len() {
            return None;
        }
        let len = u16::from_le_bytes(self.strings[off..off + 2].try_into().ok()?) as usize;
        if off + 2 + len > self.strings.len() {
            return None;
        }
        let s = &self.strings[off + 2..off + 2 + len];
        core::str::from_utf8(s).ok()
    }
}

/// Hash BLAKE3 truncado a 32 bits -- usa la implementacion nativa completa
/// de `crate::bef::blake3` y trunca a los primeros 32 bits.
pub(crate) fn blake3_hash32(bytes: &[u8]) -> u32 {
    let full = crate::bef::blake3::hash(bytes);
    u32::from_le_bytes([full[0], full[1], full[2], full[3]])
}
