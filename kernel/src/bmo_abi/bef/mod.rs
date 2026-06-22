//! `bmo_abi::bef` — Formato BEF (BMO Executable Format).
//!
//! Define el header de un archivo BEF. El loader real (que mapea
//! secciones en memoria y aplica relocalizaciones) vive en
//! `crate::bmo_core::bef::loader`.
//!
//! ## Layout
//!
//! ```text
//! ┌────────────────────────────────┐ 0
//! │ BEF Header (128 bytes)         │
//! │   magic:      "BEF\0"          │
//! │   version:    (1, 0)           │
//! │   entry:      u64              │
//! │   flags:      u32              │
//! │   ...                          │
//! ├────────────────────────────────┤ 128
//! │ .text  (código x86-64)         │
//! ├────────────────────────────────┤
//! │ .rodata                        │
//! ├────────────────────────────────┤
//! │ .data                          │
//! ├────────────────────────────────┤
//! │ .bss  (zero-init, en memoria)  │
//! ├────────────────────────────────┤
//! │ .relocs                        │
//! ├────────────────────────────────┤
//! │ .symtab (opcional)             │
//! └────────────────────────────────┘
//! ```

#![allow(dead_code)]

// ─── Magic + version ───────────────────────────────────────────────

/// Magic de 4 bytes al inicio del archivo BEF.
pub const BEF_MAGIC: [u8; 4] = *b"BEF\0";

/// Versión actual del formato BEF.
pub const BEF_VERSION: (u8, u8) = (1, 0);

// ─── Flags ─────────────────────────────────────────────────────────

/// Flags del header BEF.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BefFlags(pub u32);

impl BefFlags {
    /// El binario es **PIE** (Position Independent Executable).
    pub const PIE:        Self = Self(1 << 0);
    /// El binario tiene relocalizaciones dinámicas.
    pub const HAS_RELOCS: Self = Self(1 << 1);
    /// El binario tiene tabla de símbolos.
    pub const HAS_SYMTAB: Self = Self(1 << 2);
    /// El binario fue compilado con BMO ABI.
    pub const BMO_ABI:    Self = Self(1 << 3);
    /// El binario requiere stack ejecutable (no recomendado).
    pub const EXEC_STACK: Self = Self(1 << 4);
    /// El binario es un driver de kernel (Ring 0).
    pub const KERNEL:     Self = Self(1 << 5);
    /// El binario usa BMO GPU (compute shader).
    pub const USES_GPU:   Self = Self(1 << 6);

    #[inline]
    pub fn contains(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
}

// ─── Section type ──────────────────────────────────────────────────

/// Tipo de sección BEF.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BefSection {
    #[default]
    Null     = 0,
    Text     = 1,  // .text (código)
    Rodata   = 2,  // .rodata (constantes)
    Data     = 3,  // .data (variables inicializadas)
    Bss      = 4,  // .bss (zero-init, no en archivo)
    Relocs   = 5,  // .relocs (relocalizaciones)
    Symtab   = 6,  // .symtab (símbolos)
    Strtab   = 7,  // .strtab (strings de símbolos)
    Debug    = 8,  // .debug (info de debug)
    Note     = 9,  // .note (notas, build ID, etc)
}

// ─── Header ────────────────────────────────────────────────────────

/// Header de un archivo BEF. Tamaño: 128 bytes.
///
/// **Estructura fija, no agregar campos sin bumpear la versión BEF.**
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BefHeader {
    /// Magic "BEF\0".
    pub magic: [u8; 4],
    /// Versión BEF.
    pub version: (u8, u8),
    /// BMO ABI version que requiere este binario.
    pub abi_version: (u8, u8),
    /// Tipo de CPU objetivo.
    pub arch: BefArch,
    /// Sustrato (kernel, ring3, etc).
    pub substrate: BefSubstrate,
    /// Flags.
    pub flags: BefFlags,
    /// Tamaño del header (siempre 128 en v1.0).
    pub header_size: u16,
    /// Tamaño de cada entry de section table (32 en v1.0).
    pub section_entry_size: u16,
    /// Número de secciones.
    pub n_sections: u32,
    /// Offset de la section table desde el inicio del archivo.
    pub section_table_off: u32,
    /// Entry point (RVA).
    pub entry_rva: u64,
    /// Tamaño total de .bss (bytes zero-init que el loader reserva).
    pub bss_size: u64,
    /// Build ID (16 bytes) o 0.
    pub build_id: [u8; 16],
    /// Tamaño total del archivo en bytes.
    pub file_size: u64,
}

impl BefHeader {
    pub const SIZE: usize = 128;

    pub const fn new() -> Self {
        Self {
            magic: BEF_MAGIC,
            version: BEF_VERSION,
            abi_version: (1, 0),
            arch: BefArch::X86_64,
            substrate: BefSubstrate::Ring3,
            flags: BefFlags::empty(),
            header_size: 128,
            section_entry_size: 32,
            n_sections: 0,
            section_table_off: 0,
            entry_rva: 0,
            bss_size: 0,
            build_id: [0; 16],
            file_size: 0,
        }
    }

    /// `true` si el magic es válido.
    pub fn is_valid(&self) -> bool {
        self.magic == BEF_MAGIC
    }
}

impl BefFlags {
    /// Constructor vacío.
    pub const fn empty() -> Self { Self(0) }
}

impl Default for BefHeader {
    fn default() -> Self { Self::new() }
}

// ─── Architecture ──────────────────────────────────────────────────

/// Arquitectura objetivo.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BefArch {
    Unknown = 0,
    X86_64  = 1,
    AArch64 = 2,
    RiscV64 = 3,
    Rdna4   = 4, // GPU shaders
}

// ─── Substrate ─────────────────────────────────────────────────────

/// Dónde se ejecuta el binario.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BefSubstrate {
    /// Binario normal de userland.
    Ring3    = 0,
    /// Driver de kernel (Ring 0).
    Ring0    = 1,
    /// GPU shader (RDNA4).
    GpuRdna4 = 2,
    /// EFI boot binary.
    Efi      = 3,
}

// ─── Section entry ─────────────────────────────────────────────────

/// Entrada de la section table. Tamaño: 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BefSectionEntry {
    /// Tipo de sección.
    pub kind: BefSection,
    /// Flags (RWX).
    pub flags: u32,
    /// RVA (offset en memoria virtual del programa).
    pub rva: u64,
    /// Offset en archivo.
    pub file_off: u32,
    /// Tamaño en archivo.
    pub file_size: u32,
    /// Tamaño en memoria (puede ser > file_size si es BSS).
    pub mem_size: u32,
    /// Alineación requerida.
    pub align: u32,
    /// Nombre (null-terminated, 8 bytes máximo).
    pub name: [u8; 8],
}

impl BefSectionEntry {
    pub const SIZE: usize = 32;

    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}
