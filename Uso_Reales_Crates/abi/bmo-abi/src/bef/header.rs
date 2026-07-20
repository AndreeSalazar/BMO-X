//! Header BEF â€” 48 bytes fijos al inicio del archivo.
//!
//! Mucho mÃ¡s compacto que ELF (64 B + Phdr) o PE (264 B DOS stub + IMAGE_NT).

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64, bx_u8};
use crate::{supports_abi, BMO_ABI_VERSION};

/// Magic constant â€” `b"BEF1"` (4 bytes LE).
pub const BEF_MAGIC: bx_u32 = u32::from_le_bytes(*b"BEF1");

/// VersiÃ³n actual del formato.
pub const BEF_VERSION_MAJOR: bx_u16 = 1;
pub const BEF_VERSION_MINOR: bx_u16 = 0;

/// Magic alternativo aceptado por el loader (para devour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BefMagic {
    /// `b"BEF1"` â€” formato nativo.
    BefNative,
    /// `b"MZ"` â€” PE/COFF de Windows (debe procesarse por `loader::pe`).
    PeWindows,
    /// `0x7F b"ELF"` â€” ELF de Linux/Unix (procesar por `loader::elf`).
    ElfUnix,
    /// Formato desconocido.
    Unknown,
}

impl BefMagic {
    /// Detecta el formato a partir de los primeros bytes.
    pub fn detect(bytes: &[u8]) -> Self {
        if bytes.len() < 4 {
            return Self::Unknown;
        }
        match &bytes[..4] {
            b"BEF1" => Self::BefNative,
            b"\x7FELF" => Self::ElfUnix,
            _ if &bytes[..2] == b"MZ" => Self::PeWindows,
            _ => Self::Unknown,
        }
    }
}

bitflags::bitflags! {
    /// Flags del header BEF.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BefFlags: bx_u32 {
        /// El binario es ejecutable (vs. librerÃ­a compartida).
        const EXECUTABLE         = 1 << 0;
        /// Es una librerÃ­a compartida (.bef.so equivalent).
        const SHARED_LIBRARY     = 1 << 1;
        /// Tiene secciÃ³n de manifest TOML vÃ¡lida.
        const HAS_MANIFEST       = 1 << 2;
        /// Tiene secciÃ³n de shaders/IR pre-compilados.
        const HAS_SHADERS        = 1 << 3;
        /// Tiene secciones comprimidas con GDeflate.
        const COMPRESSED         = 1 << 4;
        /// EstÃ¡ firmado con Ed25519.
        const SIGNED             = 1 << 5;
        /// Posee TLS (Thread Local Storage).
        const HAS_TLS            = 1 << 6;
        /// Soporta hot-reload de secciones en runtime.
        const HOT_RELOADABLE     = 1 << 7;
        /// Marca PIE (Position Independent Executable) â€” siempre verdadero
        /// para BEF nativo, pero Ãºtil para PE/ELF devorados.
        const PIE                = 1 << 8;
        /// Importa la API BareX (vs solo usar syscalls BMO).
        const USES_BAREX         = 1 << 9;
        /// Origen: PE devorado (set por el loader, no por el compilador).
        const PROVENANCE_PE      = 1 << 14;
        /// Origen: ELF devorado.
        const PROVENANCE_ELF     = 1 << 15;
    }
}

/// Arquitectura target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BefArch {
    /// Reservado / desconocido.
    Reserved = 0x00,
    /// AMD64 / x86-64 (baseline BMO).
    X86_64 = 0x01,
    /// AArch64 (futuro, no implementado).
    Aarch64 = 0x02,
    /// RISC-V 64-bit (futuro).
    Rv64gc = 0x03,
}

/// Header BEF â€” 48 bytes, alineado a 16.
///
/// Campos pequeÃ±os primero (BMO ABI struct layout).
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct BefHeader {
    /// `BEF_MAGIC` = `BEF1` LE.
    pub magic: bx_u32,
    /// VersiÃ³n del formato.
    pub version_major: bx_u16,
    pub version_minor: bx_u16,
    /// Flags `BefFlags`.
    pub flags: bx_u32,
    /// Arquitectura `BefArch as u8`, padding 3 bytes.
    pub arch: bx_u8,
    pub _pad0: [bx_u8; 3],
    /// VersiÃ³n del BMO ABI esperado (major, minor).
    pub abi_version_major: bx_u8,
    pub abi_version_minor: bx_u8,
    pub _pad1: [bx_u8; 6],
    /// Offset del entry point dentro de la secciÃ³n `.code`.
    pub entry_offset: bx_u64,
    /// Offset absoluto del section table.
    pub section_table_offset: bx_u64,
    /// Cantidad de secciones.
    pub section_count: bx_u32,
    /// TamaÃ±o total del archivo en bytes (validaciÃ³n rÃ¡pida).
    pub total_size: bx_u32,
}

impl BefHeader {
    pub const SIZE: usize = 48;
    const _SIZE_ASSERT: () = assert!(core::mem::size_of::<BefHeader>() == 48);

    /// Construye el header default para un ejecutable BEF v1.0 nativo.
    pub const fn new_executable() -> Self {
        Self {
            magic: BEF_MAGIC,
            version_major: BEF_VERSION_MAJOR,
            version_minor: BEF_VERSION_MINOR,
            flags: BefFlags::EXECUTABLE.bits() | BefFlags::PIE.bits() | BefFlags::USES_BAREX.bits(),
            arch: BefArch::X86_64 as u8,
            _pad0: [0; 3],
            abi_version_major: BMO_ABI_VERSION.0,
            abi_version_minor: BMO_ABI_VERSION.1,
            _pad1: [0; 6],
            entry_offset: 0,
            section_table_offset: 0,
            section_count: 0,
            total_size: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == BEF_MAGIC
            && self.version_major == BEF_VERSION_MAJOR
            && supports_abi((self.abi_version_major, self.abi_version_minor))
            && self.section_count > 0
            && self.section_count < 256
    }
}
