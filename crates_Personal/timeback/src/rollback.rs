//! `timeback::rollback` — Reversión a un checkpoint.

#![allow(dead_code)]

use core::sync::atomic::Ordering;

use super::checkpoint::CheckpointId;
use super::storage;
use super::CURRENT_EPOCH;

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
/// v1.9: Validates that the checkpoint exists in our name table and
/// updates the current epoch. Actual state restoration (heap, procs) is
/// a future task; for now we signal success and let the caller re-initialize.
pub fn to(id: CheckpointId) -> RollbackResult {
    if super::checkpoint::name(id).is_none() {
        return RollbackResult::NotFound;
    }

    // We can't restore heap/proc state generically in v1.9, but we can
    // signal that the checkpoint is valid and update the epoch to mark
    // the rollback event.
    CURRENT_EPOCH.fetch_add(1, Ordering::SeqCst);

    // Touch the NVRAM to mark this checkpoint as the active one.
    let _ = storage::persist_to_nvram(id.0, b"ROLLBACK");

    RollbackResult::Ok
}
