//! TimeBack: BMO system Git-like version control for kernel state.
//!
//! Like Git, but for kernel snapshots, driver configs, and system state.
//! Persists to SSD (T: partition) with NVRAM fallback for crash-safety.
//!
//! ## v1.9: full Git-like API
//!
//! - **Objects**: commits, trees, blobs (content-addressed by FNV-1a hash)
//! - **Refs**: branches, tags, HEAD
//! - **Index**: staging area
//! - **CLI**: `tb init`, `tb add`, `tb commit`, `tb log`, `tb branch`,
//!   `tb checkout`, `tb diff`, `tb save`, `tb restore`
//!
//! ## v1.8.x: legacy API (kept for compatibility)
//!
//! - `create_checkpoint(name)` — names a checkpoint
//! - `rollback(id)` — restores
//!
//! ## Golden rule
//!
//! - TimeBack **does not decide security policies** (that's ByteDefender).
//! - Cabina can request a rollback from the HUD.
//! - ByteDefender can create a checkpoint before executing an app.

#![no_std]

extern crate alloc;

pub mod blob;
pub mod checkpoint;
pub mod cli;
pub mod commit;
pub mod delta;
pub mod hash;
pub mod journal;
pub mod policy;
pub mod r#ref;
pub mod repo;
pub mod rollback;
pub mod snapshot;
pub mod storage;
pub mod tree;

pub use blob::Blob;
pub use commit::{Author, Commit};
pub use hash::Hash;
pub use r#ref::{RefEntry, RefKind};
pub use repo::DiffOp;
pub use tree::{FileMode, Tree, TreeEntry};

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

// ── Legacy v1.8.x API (kept for compatibility) ─────────────────────

/// Create a named checkpoint. Returns the ID.
/// (Legacy API — prefer `repo::commit()`)
pub fn create_checkpoint(name: &str) -> checkpoint::CheckpointId {
    let id = checkpoint::create(name);
    CURRENT_EPOCH.fetch_add(1, Ordering::SeqCst);
    id
}

/// Auto-commit: called periodically (e.g. every 1s from the timer tick).
/// Creates a checkpoint tagged "auto" if at least `min_secs` have elapsed
/// since the last auto-commit. Returns Some(id) if a checkpoint was made.
pub fn auto_commit_if_due(min_secs: u64) -> Option<checkpoint::CheckpointId> {
    let now = read_tick_ns();
    let last = LAST_AUTO_NS.load(Ordering::SeqCst);
    if last != 0 && now.saturating_sub(last) < min_secs * 1_000_000_000 { return None; }
    LAST_AUTO_NS.store(now, Ordering::SeqCst);

    // Stage a synthetic snapshot blob and create a Git commit too
    let snap = snapshot::Snapshot::capture();
    let mut buf = [0u8; 64];
    let mut p = 0;
    buf[p..p+8].copy_from_slice(&snap.epoch.to_le_bytes()); p += 8;
    buf[p..p+8].copy_from_slice(&snap.tick_ns.to_le_bytes()); p += 8;
    buf[p..p+8].copy_from_slice(&snap.heap_used.to_le_bytes()); p += 8;
    buf[p..p+4].copy_from_slice(&snap.running_processes.to_le_bytes()); p += 4;
    buf[p..p+4].copy_from_slice(&snap.open_files.to_le_bytes()); p += 4;
    // Stage and commit to repo (if initialized)
    if repo::is_initialized() {
        repo::add("auto.snap", &buf[..p]);
        if let Some(_h) = repo::commit("auto", Author::kernel()) {
            // Success
        }
    }
    Some(checkpoint::create("auto"))
}

/// Revert the system to a checkpoint. Returns the result.
/// (Legacy API)
pub fn rollback(id: checkpoint::CheckpointId) -> rollback::RollbackResult {
    rollback::to(id)
}

static LAST_AUTO_NS: AtomicU64 = AtomicU64::new(0);

/// Read the kernel's monotonic tick counter.
fn read_tick_ns() -> u64 {
    let cb = unsafe { TICK_SOURCE };
    if let Some(f) = cb { return f(); }
    SOFTWARE_TICKS.fetch_add(1_000_000, Ordering::Relaxed)
}

static mut TICK_SOURCE: Option<fn() -> u64> = None;
static SOFTWARE_TICKS: AtomicU64 = AtomicU64::new(0);

/// Register a function that returns monotonic nanoseconds.
pub fn set_tick_source(f: fn() -> u64) {
    unsafe { TICK_SOURCE = Some(f); }
}

// ── Legacy re-exports ─────────────────────────────────────────────

pub use checkpoint::CheckpointId;
pub use snapshot::Snapshot;
pub use delta::Delta;
pub use journal::{JournalEntry, JournalOp};
pub use rollback::RollbackResult;
