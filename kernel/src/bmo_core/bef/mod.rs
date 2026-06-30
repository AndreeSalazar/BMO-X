//! BEF — formato ejecutable universal de FastOS.
//!
//! ## Filosofía
//!
//! BEF es el ÚNICO formato ejecutable nativo de FastOS. Pero también
//! **devora** PE (Windows .exe/.dll) y ELF (Linux/Unix), traduciéndolos
//! transparentemente a representación BEF interna en tiempo de carga.
//!
//! Una vez parseados, todos viven bajo la misma representación
//! ([`Image`]), todos respetan el modelo de capabilities BEF y el BMO ABI.
//!
//! Spec maestra: `MAPA de Window/02_BEF_Format/BEF_Executable_Format_Spec.md`.
//! Mapa de carpetas y devour-strategy: `_README.md` en este folder.
//!
//! ## Lo que ningún otro formato tiene
//!
//! - Shaders/IR **pre-compilados** integrados en `.shaders` (cero stutter).
//! - Manifest TOML con **capabilities declarativas** (sandbox por construcción).
//! - **Hash BLAKE3** por sección (verificación al cargar, ~1 GB/s).
//! - **TLS layout BMO** (sin `.tdata`/`.tbss` separados como ELF; un solo blob).
//! - **Imports/exports** referenciados por `BmoHandle` con generación.
//! - **Cero secciones legacy** (sin `.eh_frame`, sin `.note.*`, sin `.comment`...).

#![allow(dead_code)]

pub use crate::bmo_abi::bef::{header, sections, imports, exports, relocations, symbols, manifest, signing, tls, blake3};
pub mod loader;
pub mod compat;

// Re-exports planos para uso ergonómico.

/// Inicializa el subsistema BEF. v1.7.4: no-op.
pub fn init() {
    // v2.0: precargar built-in BEFs desde ramdisk.
}
