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

pub mod header;
pub mod sections;
pub mod manifest;
pub mod signing;
pub mod relocations;
pub mod symbols;
pub mod tls;
pub mod imports;
pub mod exports;
pub mod blake3;
pub mod writer;
pub mod validator;
pub mod loader;

// ─── Re-exports del loader canónico ─────────────────────────────────
pub use header::{
    BEF_MAGIC, BEF_VERSION_MAJOR, BEF_VERSION_MINOR,
    BefMagic, BefFlags, BefArch, BefHeader,
};

/// Versión del formato BEF como tupla `(major, minor)`.
pub const BEF_VERSION: (u16, u16) = (BEF_VERSION_MAJOR, BEF_VERSION_MINOR);

// ─── Re-exports de sections ─────────────────────────────────────────
pub use sections::{
    SectionKind, SectionFlags, SectionEntry, SectionTable,
};

// ─── Re-exports de manifest, signing, relocations, symbols, etc ────
pub use manifest::Provenance;
pub use signing::{SignatureHeader, SectionHash};
pub use relocations::Relocation;
pub use symbols::Symbol;
pub use tls::TlsTemplate;
pub use imports::ImportEntry;
pub use exports::ExportEntry;
pub use writer::BefBuilder;
pub use validator::validate;
pub use loader::{load, LoadedBef, LoadedSection};
