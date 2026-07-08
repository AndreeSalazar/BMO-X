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

/// Auto-commit: called periodically (e.g. every 1s from the timer tick).
/// Creates a checkpoint tagged "auto" if at least `min_secs` have elapsed
/// since the last auto-commit. Returns Some(id) if a checkpoint was made.
pub fn auto_commit_if_due(min_secs: u64) -> Option<CheckpointId> {
    let now = read_tick_ns();
    let last = LAST_AUTO_NS.load(Ordering::SeqCst);
    if last != 0 && now.saturating_sub(last) < min_secs * 1_000_000_000 { return None; }
    LAST_AUTO_NS.store(now, Ordering::SeqCst);
    Some(create_checkpoint("auto"))
}

/// Revert the system to a checkpoint. Returns the result.
pub fn rollback(id: CheckpointId) -> RollbackResult {
    rollback::to(id)
}

static LAST_AUTO_NS: AtomicU64 = AtomicU64::new(0);

/// Read the kernel's monotonic tick counter. Implemented by the kernel via
/// the HAL; defaults to a software counter for testing.
fn read_tick_ns() -> u64 {
    // The kernel can register a tick_ns source via set_tick_source().
    let cb = unsafe { TICK_SOURCE };
    if let Some(f) = cb { return f(); }
    // Fallback: use an AtomicU64 incremented in software
    SOFTWARE_TICKS.fetch_add(1_000_000, Ordering::Relaxed)
}

static mut TICK_SOURCE: Option<fn() -> u64> = None;
static SOFTWARE_TICKS: AtomicU64 = AtomicU64::new(0);

/// Register a function that returns monotonic nanoseconds.
/// The kernel calls this once during `timeback::init()`.
pub fn set_tick_source(f: fn() -> u64) {
    unsafe { TICK_SOURCE = Some(f); }
}
