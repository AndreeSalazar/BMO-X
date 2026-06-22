//! `bmo_abi::bef` — Formato BEF (BMO Executable Format).
//!
//! v1.8.8: este módulo es un **re-export** de los tipos canónicos
//! definidos en `crate::bmo_core::bef`. La fuente única de verdad
//! para BEF es el módulo del loader (que ya tiene 9 archivos:
//! header, sections, manifest, signing, etc.).
//!
//! Antes había una **duplicación** con valores distintos (magic
//! `BEF\0` aquí vs `BEF1` allá, header 128 B vs 48 B). Ahora todo
//! viene del loader.
//!
//! ## Layout
//!
//! ```text
//! ┌────────────────────────────────┐ 0
//! │ BEF Header (48 bytes)          │
//! │   magic:      "BEF1" LE        │
//! │   version:    (1, 0)           │
//! │   flags:      BefFlags (u32)   │
//! │   arch:       BefArch          │
//! │   ...                          │
//! ├────────────────────────────────┤ 48
//! │ .code  (código x86-64)         │
//! │ .rodata                        │
//! │ .data / .bss                   │
//! │ .relocs / .symbols / .manifest │
//! │ .shaders / .resources          │
//! │ .signature (BLAKE3 + Ed25519)  │
//! └────────────────────────────────┘
//! ```

#![allow(dead_code)]

// ─── Re-exports del loader canónico ─────────────────────────────────
pub use crate::bmo_core::bef::header::{
    BEF_MAGIC, BEF_VERSION_MAJOR, BEF_VERSION_MINOR,
    BefMagic, BefFlags, BefArch, BefHeader,
};

/// Versión del formato BEF como tupla `(major, minor)`.
pub const BEF_VERSION: (u16, u16) = (BEF_VERSION_MAJOR, BEF_VERSION_MINOR);

// ─── Re-exports de sections ─────────────────────────────────────────
pub use crate::bmo_core::bef::sections::{
    SectionKind, SectionFlags, SectionEntry, SectionTable,
};

// ─── Re-exports de manifest, signing, relocations, symbols, etc ────
pub use crate::bmo_core::bef::manifest::Provenance;
pub use crate::bmo_core::bef::signing::{SignatureHeader, SectionHash};
pub use crate::bmo_core::bef::relocations::Relocation;
pub use crate::bmo_core::bef::symbols::Symbol;
pub use crate::bmo_core::bef::tls::TlsTemplate;
pub use crate::bmo_core::bef::imports::ImportEntry;
pub use crate::bmo_core::bef::exports::ExportEntry;
