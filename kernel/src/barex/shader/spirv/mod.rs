//! SPIR-V 1.6 → SASS. Delega a `naga` (Rust nativo, ya en ecosistema).
//!
//! `naga` parsea SPIR-V y produce su IR intermedio; de ahí se pasa a NAK
//! (NVK Assembler/Kompiler) para emitir SASS sm_86 (GA106).

use crate::barex::{BxError, BxResult};
extern crate alloc;
use alloc::vec::Vec;

/// Traduce un blob SPIR-V 1.6 a SASS GA106. Vacío hasta que se agregue
/// `naga` como dep del kernel (o se invoque desde Ring 3 vía `barexc`).
pub fn translate_to_sass(_spirv: &[u8]) -> BxResult<Vec<u8>> {
    Err(BxError::NotImplemented)
}
