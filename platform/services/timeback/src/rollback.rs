//! `timeback::rollback` — Reversión a un checkpoint (legacy API).

#![allow(dead_code)]

use core::sync::atomic::Ordering;

use super::checkpoint::CheckpointId;
use super::storage;
use super::CURRENT_EPOCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollbackResult {
    Ok,
    NotFound,
    Failed,
}

pub fn to(id: CheckpointId) -> RollbackResult {
    if super::checkpoint::name(id).is_none() {
        return RollbackResult::NotFound;
    }
    CURRENT_EPOCH.fetch_add(1, Ordering::SeqCst);
    let _ = storage::persist_to_nvram(id.0, b"ROLLBACK");
    RollbackResult::Ok
}
