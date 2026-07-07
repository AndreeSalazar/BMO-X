//! TimeBack: BMO system time-travel / checkpoint-rollback subsystem.
//!
//! v1.8.8: sibling of `cabina` and `defense` (the "Trilogy").
//! TimeBack enables reverting system state to a previous checkpoint:
//!
//! - **Checkpoints**: named return points.
//! - **Snapshots**: system state at an instant.
//! - **Deltas**: incremental changes between snapshots.
//! - **Journal**: operation log for replay or revert.
//! - **Rollback**: return to a prior checkpoint.
//!
//! ## Golden rule
//!
//! - TimeBack **does not decide security policies** (that's ByteDefender).
//! - Cabina can request a rollback from the HUD.
//! - ByteDefender can create a checkpoint before executing an app.
//!
//! ## v1.8.8: status
//!
//! - API complete (stubs).
//! - Storage in RAM (no SSD/FS yet).
//! - Journal in ring buffer.

#![no_std]

extern crate alloc;

pub mod checkpoint;
pub mod snapshot;
pub mod delta;
pub mod journal;
pub mod rollback;
pub mod storage;
pub mod policy;

#[cfg(test)]
mod tests;

pub use checkpoint::CheckpointId;
pub use snapshot::Snapshot;
pub use delta::Delta;
pub use journal::{JournalEntry, JournalOp};
pub use rollback::RollbackResult;

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static INIT: AtomicBool = AtomicBool::new(false);
static CURRENT_EPOCH: AtomicU64 = AtomicU64::new(0);

/// Initialize TimeBack. Call once at boot.
pub fn init() {
    if INIT.swap(true, Ordering::SeqCst) { return; }
    storage::init();
    journal::init();
    policy::init();
}

/// Current epoch (monotonically increasing). Incremented on each checkpoint.
pub fn current_epoch() -> u64 { CURRENT_EPOCH.load(Ordering::SeqCst) }

/// Create a named checkpoint. Returns the ID.
pub fn create_checkpoint(name: &str) -> CheckpointId {
    let id = checkpoint::create(name);
    CURRENT_EPOCH.fetch_add(1, Ordering::SeqCst);
    id
}

/// Revert the system to a checkpoint. Returns the result.
pub fn rollback(id: CheckpointId) -> RollbackResult {
    rollback::to(id)
}
