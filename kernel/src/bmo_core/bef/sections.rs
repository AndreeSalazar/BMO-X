//! Tabla de secciones BEF.
//!
//! 10 tipos de sección, cada una con propósito claro. Comparado con ELF
//! (~20 tipos) y PE (~11 tipos) — más limitado pero más limpio: cada
//! sección tiene una semántica única y obligatoria, no es solo "datos".

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::{bx_u8, bx_u16, bx_u32, bx_u64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SectionKind {
    /// Código ejecutable x86-64. Cargada como RX.
    Code      = 0x01,
    /// Datos inicializados de solo lectura (rodata). Cargada como R.
    RoData    = 0x02,
    /// Datos inicializados mutables. Cargada como RW.
    Data      = 0x03,
    /// Datos no inicializados (BSS). Reservado en RAM, no en archivo.
    Bss       = 0x04,
    /// Tabla de imports (lazy/eager binding via BMO).
    Imports   = 0x05,
    /// Tabla de exports (símbolos visibles desde fuera).
    Exports   = 0x06,
    /// Tabla de relocations.
    Relocs    = 0x07,
    /// Tabla de símbolos (debug + dynamic linking).
    Symbols   = 0x08,
    /// Manifest TOML (capabilities, version, dependencies).
    Manifest  = 0x09,
    /// Shaders/IR pre-compilados.
    Shaders   = 0x0A,
    /// Recursos arbitrarios (texturas BC7, audio Opus, fonts, etc.).
    Resources = 0x0B,
    /// Layout de TLS (Thread Local Storage).
    Tls       = 0x0C,
    /// Stack unwind info para excepciones / backtraces.
    Unwind    = 0x0D,
    /// Debug info (DWARF-lite específico de BEF).
    Debug     = 0x0E,
    /// Hashes BLAKE3 + firma Ed25519 opcional.
    Signature = 0x0F,

    // ─── Sesión 8: secciones de metadatos genéricos multi-lenguaje ────
    /// Tabla de `TypeDescriptor` (consumida por `bmo_gpu::abi::type_system::TypeRegistry`).
    TypeMap   = 0x10,
    /// VTables `BmoVTable` empacadas (`bmo_gpu::abi::vtable`).
    VTables   = 0x11,
    /// Bridges de lenguaje origen (`bmo_gpu::abi::lang_bridge::LangDescriptor`).
    LangBridge = 0x12,
    /// Datos de reflection (mirrors, nombres mangled extra).
    Reflect   = 0x13,
    /// Tabla de cierres `BmoClosure` con `ClosureSig`.
    Closures  = 0x14,
}

impl SectionKind {
    pub fn from_u8(v: bx_u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Code),
            0x02 => Some(Self::RoData),
            0x03 => Some(Self::Data),
            0x04 => Some(Self::Bss),
            0x05 => Some(Self::Imports),
            0x06 => Some(Self::Exports),
            0x07 => Some(Self::Relocs),
            0x08 => Some(Self::Symbols),
            0x09 => Some(Self::Manifest),
            0x0A => Some(Self::Shaders),
            0x0B => Some(Self::Resources),
            0x0C => Some(Self::Tls),
            0x0D => Some(Self::Unwind),
            0x0E => Some(Self::Debug),
            0x0F => Some(Self::Signature),
            0x10 => Some(Self::TypeMap),
            0x11 => Some(Self::VTables),
            0x12 => Some(Self::LangBridge),
            0x13 => Some(Self::Reflect),
            0x14 => Some(Self::Closures),
            _ => None,
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SectionFlags: bx_u32 {
        /// Mapear como readable.
        const READ          = 1 << 0;
        /// Mapear como writable.
        const WRITE         = 1 << 1;
        /// Mapear como executable.
        const EXEC          = 1 << 2;
        /// Sección comprimida con GDeflate (descomprimir al cargar).
        const COMPRESSED    = 1 << 3;
        /// Sección requiere alineación a página (4 KB).
        const PAGE_ALIGNED  = 1 << 4;
        /// Sección requiere alineación a huge page (2 MB).
        const HUGE_ALIGNED  = 1 << 5;
        /// Lazy: no cargar hasta que se use (file-backed).
        const LAZY          = 1 << 6;
        /// Sección verificada por hash al cargar.
        const HASHED        = 1 << 7;
        /// Sección sintetizada por el devour-loader (no estaba en el archivo).
        const SYNTHETIC     = 1 << 8;
    }
}

/// Una entrada del section table — 48 bytes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct SectionEntry {
    /// `SectionKind as u8`.
    pub kind: bx_u8,
    /// Padding.
    pub _pad: [bx_u8; 3],
    /// `SectionFlags`.
    pub flags: bx_u32,
    /// Offset dentro del archivo (0 si es BSS o sintética).
    pub file_offset: bx_u64,
    /// Tamaño en archivo.
    pub file_size: bx_u64,
    /// Tamaño en memoria (≥ file_size; diferencia es zero-fill).
    pub mem_size: bx_u64,
    /// Dirección virtual deseada (0 = elige el loader).
    pub virt_addr: bx_u64,
    /// Alineación requerida (potencia de 2, default 8).
    pub alignment: bx_u16,
    /// Índice de la sección Signature que contiene su hash, o 0xFFFF si no aplica.
    pub hash_index: bx_u16,
    /// Reservado.
    pub _reserved: bx_u32,
}

impl SectionEntry {
    pub const SIZE: usize = 48;

    pub const ZERO: Self = Self {
        kind: 0,
        _pad: [0; 3],
        flags: 0,
        file_offset: 0,
        file_size: 0,
        mem_size: 0,
        virt_addr: 0,
        alignment: 8,
        hash_index: 0xFFFF,
        _reserved: 0,
    };

    pub fn kind(&self) -> Option<SectionKind> {
        SectionKind::from_u8(self.kind)
    }
}

/// Vista in-memory del section table. Cero-copy sobre los bytes del archivo.
pub struct SectionTable<'a> {
    pub entries: &'a [SectionEntry],
}

impl<'a> SectionTable<'a> {
    pub fn parse(bytes: &'a [u8], offset: u64, count: u32) -> Result<Self, &'static str> {
        let off = offset as usize;
        let needed = count as usize * SectionEntry::SIZE;
        if off + needed > bytes.len() {
            return Err("section table fuera de rango");
        }
        let raw_ptr = unsafe { bytes.as_ptr().add(off) };
        if (raw_ptr as usize) % core::mem::align_of::<SectionEntry>() != 0 {
            return Err("section table pointer mal alineado");
        }
        let ptr = raw_ptr as *const SectionEntry;
        let entries = unsafe { core::slice::from_raw_parts(ptr, count as usize) };
        Ok(Self { entries })
    }

    pub fn find(&self, kind: SectionKind) -> Option<&SectionEntry> {
        self.entries.iter().find(|e| e.kind == kind as u8)
    }
}
