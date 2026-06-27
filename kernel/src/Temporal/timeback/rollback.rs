//! `timeback::rollback` — Reversión a un checkpoint.

#![allow(dead_code)]

use super::checkpoint::CheckpointId;

/// Resultado de un rollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollbackResult {
    /// Rollback exitoso.
    Ok,
    /// Checkpoint no encontrado.
    NotFound,
    /// No se puede hacer rollback (p.ej. storage corrupto).
    Failed,
}

/// Revierte el sistema a un checkpoint.
pub fn to(_id: CheckpointId) -> RollbackResult {
    // v1.8.8: stub. En v1.9 implementaremos la reversión real.
    RollbackResult::Ok
}
