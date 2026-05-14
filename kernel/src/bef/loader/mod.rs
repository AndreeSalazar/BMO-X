//! Loader BEF — entry point único que devora 3 formatos: BEF, PE, ELF.
//!
//! ```text
//!   bef::load(bytes) ──▶ detect_format ─┬──▶ native::load (BEF)
//!                                       ├──▶ pe::load     (Windows .exe/.dll)
//!                                       └──▶ elf::load    (Linux/Unix)
//!                              │
//!                              ▼
//!                          Image (representación BEF unificada)
//! ```

#![allow(dead_code)]

extern crate alloc;

pub mod native;
pub mod pe;
pub mod elf;

use crate::bef::header::BefMagic;
use crate::bef::manifest::{Manifest, Provenance};

/// Formato detectado del binario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFormat {
    /// BEF nativo de FastOS.
    BefNative,
    /// PE devorado y traducido a BEF interno.
    PeDevoured,
    /// ELF devorado y traducido a BEF interno.
    ElfDevoured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    UnknownFormat,
    Truncated,
    InvalidHeader,
    UnsupportedArch,
    UnsupportedAbi,
    SectionOutOfRange,
    HashMismatch,
    NotImplemented,
}

/// Imagen cargada en memoria — representación BEF unificada (cualquier
/// origen acaba aquí).
pub struct Image {
    pub format: BinaryFormat,
    pub manifest: Manifest,
    pub entry_point: u64,
    pub base_address: u64,
    pub sections: alloc::vec::Vec<MappedSection>,
}

#[derive(Debug, Clone, Copy)]
pub struct MappedSection {
    pub kind: u8,           // SectionKind as u8
    pub virt_addr: u64,
    pub size: u64,
    pub flags: u32,
}

/// Punto de entrada universal — detecta formato y delega al sub-loader.
pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    match BefMagic::detect(bytes) {
        BefMagic::BefNative => native::load(bytes),
        BefMagic::PeWindows => pe::load(bytes),
        BefMagic::ElfUnix   => elf::load(bytes),
        BefMagic::Unknown   => Err(LoadError::UnknownFormat),
    }
}

/// Helper compartido — sintetiza una `MappedSection` vacía.
pub(crate) fn placeholder_section(kind: u8) -> MappedSection {
    MappedSection { kind, virt_addr: 0, size: 0, flags: 0 }
}

pub(crate) fn fake_provenance_image(prov: Provenance) -> Image {
    Image {
        format: match prov {
            Provenance::Native      => BinaryFormat::BefNative,
            Provenance::PeDevoured  => BinaryFormat::PeDevoured,
            Provenance::ElfDevoured => BinaryFormat::ElfDevoured,
        },
        manifest: Manifest::synthetic_for("(stub)", prov),
        entry_point: 0,
        base_address: 0,
        sections: alloc::vec::Vec::new(),
    }
}
