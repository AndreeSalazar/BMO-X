//! SPIR-V 1.6 → IR/backend nativo. Delega a `naga` cuando se conecte.
//!
//! `naga` parsea SPIR-V y produce su IR intermedio. El backend inicial de
//! FastOS puede validar/interpretar sobre GOP/software; un backend acelerado
//! futuro podrá emitir código nativo propio.

use crate::barex::{BxError, BxResult};
extern crate alloc;
use alloc::vec::Vec;

/// Traduce un blob SPIR-V 1.6 a IR/backend nativo. Vacío hasta que se agregue
/// `naga` como dep del kernel (o se invoque desde Ring 3 vía `barexc`).
pub fn translate_to_native(_spirv: &[u8]) -> BxResult<Vec<u8>> {
    Err(BxError::NotImplemented)
}
