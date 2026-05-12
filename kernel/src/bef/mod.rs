//! BEF — formato ejecutable nativo de FastOS.
//!
//! Spec: `BEF_Executable_Format_Spec.md` (v1.1 con shaders nativos).
//! Reemplaza ELF y PE para apps FastOS. Transporta:
//!   - Código x86-64 nativo
//!   - Shaders SASS GA106 pre-compilados (sección `.shaders`)
//!   - Manifiesto TOML con capabilities (network, gpu, fs, input)
//!   - Recursos (texturas BC7/ASTC, audio Opus, fonts, etc.)

#![allow(dead_code)]

pub const BEF_MAGIC: &[u8; 4] = b"BEF1";

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BefHeader {
    pub magic: [u8; 4],
    pub version_major: u16,
    pub version_minor: u16,
    pub flags: u32,
    pub entry_offset: u64,
    pub section_table_offset: u64,
    pub section_count: u32,
    pub _reserved: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Code,
    Data,
    Shaders,
    Manifest,
    Resources,
    Debug,
}

/// Esqueleto de loader. La implementación completa parsea, valida hashes
/// y monta el ejecutable en un nuevo address space.
pub fn load(_bytes: &[u8]) -> Result<(), &'static str> {
    Err("bef::load no implementado todavía")
}
