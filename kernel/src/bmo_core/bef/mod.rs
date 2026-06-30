//! BEF — formato ejecutable universal de FastOS.
//!
//! Arquitectura:
//! ┌─────────────────────────────────────────────┐
//! │  compact/    definiciones del formato        │
//! │  loader/     parseo individual (BEF/PE/ELF)  │
//! │  devour/     orquestador (bytes → Image)     │
//! │  compat/     shims Win32/Linux               │
//! └─────────────────────────────────────────────┘

#![allow(dead_code)]

pub mod format;
pub mod parsers;
pub mod devour;
pub mod shims;

// Re-exports planos desde compact/ para uso ergonómico
pub use format::{header, sections, imports, exports, relocations, symbols, manifest, signing, tls, blake3};

pub fn init() {
    // v2.0: precargar built-in BEFs desde ramdisk.
}
