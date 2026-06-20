//! Snapshot storage and management

#![allow(dead_code, unused_unsafe)]

use super::Snapshot;

/// Get snapshot by index
pub fn get_snapshot(index: usize) -> Option<&'static Snapshot> {
    unsafe {
        let count = super::state().snapshot_count.min(super::MAX_SNAPSHOTS as u64) as usize;
        if index < count {
            Some(&super::state().snapshots[index])
        } else {
            None
        }
    }
}

/// Find snapshot by ID
pub fn find_by_id(id: u64) -> Option<&'static Snapshot> {
    super::find_snapshot(id)
}
