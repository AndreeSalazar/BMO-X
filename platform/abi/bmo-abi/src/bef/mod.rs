//! `bmo_abi::bef` -- Formato BEF (BMO Executable Format).
//!
//! v1.8.8: este modulo es un **re-export** de los tipos canonicos
//! definidos en `crate::bmo_core::bef`. La fuente unica de verdad
//! para BEF es el modulo del loader (que ya tiene 9 archivos:
//! header, sections, manifest, signing, etc.).
//!
//! Antes habia una **duplicacion** con valores distintos (magic
//! `BEF\0` aqui vs `BEF1` alla, header 128 B vs 48 B). Ahora todo
//! viene del loader.
//!
//! ## Layout
//!
//! ```text
//! +--------------------------------+ 0
//! | BEF Header (48 bytes)          |
//! |   magic:      "BEF1" LE        |
//! |   version:    (1, 0)           |
//! |   flags:      BefFlags (u32)   |
//! |   arch:       BefArch          |
//! |   ...                          |
//! +--------------------------------+ 48
//! | .code  (codigo x86-64)         |
//! | .rodata                        |
//! | .data / .bss                   |
//! | .relocs / .symbols / .manifest |
//! | .shaders / .resources          |
//! | .signature (BLAKE3 + Ed25519)  |
//! +--------------------------------+
//! ```

#![allow(dead_code)]

pub mod blake3;
pub mod exports;
pub mod header;
pub mod imports;
pub mod linker;
pub mod loader;
pub mod manifest;
pub mod paquete;
pub mod katanas;
pub mod recursos;
pub mod relocations;
pub mod requisitos;
pub mod sections;
pub mod signing;
pub mod symbols;
pub mod tls;
pub mod validator;
pub mod writer;

// --- Re-exports del loader canonico ---------------------------------
pub use header::{
    BefArch, BefFlags, BefHeader, BefMagic, BEF_MAGIC, BEF_VERSION_MAJOR, BEF_VERSION_MINOR,
};

/// Version del formato BEF como tupla `(major, minor)`.
pub const BEF_VERSION: (u16, u16) = (BEF_VERSION_MAJOR, BEF_VERSION_MINOR);

// --- Re-exports de sections -----------------------------------------
pub use sections::{SectionEntry, SectionFlags, SectionKind, SectionTable};

// --- Re-exports de manifest, signing, relocations, symbols, etc ----
pub use exports::ExportEntry;
pub use imports::ImportEntry;
pub use loader::{load, LoadedBef, LoadedSection};
pub use manifest::Provenance;
pub use paquete::{directorio, empaquetar, localizar_recursos};
pub use recursos::{Directorio, Entrada as EntradaRecurso, RECURSOS_MAGIC};
pub use relocations::Relocation;
pub use requisitos::{
    Declaracion as DeclaracionRequisito, Requisito, Tabla as TablaRequisitos, REQUISITOS_MAGIC,
};
pub use signing::{SectionHash, SignatureHeader};
pub use symbols::Symbol;
pub use tls::TlsTemplate;
pub use validator::validate;
pub use writer::{BefBuilder, BefSection};
