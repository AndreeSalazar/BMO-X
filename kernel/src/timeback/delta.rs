//! `timeback::delta` — Diff entre dos snapshots.

#![allow(dead_code)]

use super::snapshot::Snapshot;

/// Delta entre dos snapshots.
#[derive(Clone, Copy, Debug, Default)]
pub struct Delta {
    pub heap_used_diff: i64,
    pub processes_diff: i32,
    pub files_diff: i32,
    pub tick_elapsed_ns: u64,
}

impl Delta {
    /// Calcula el delta entre `a` (anterior) y `b` (nuevo).
    pub fn between(a: &Snapshot, b: &Snapshot) -> Self {
        Self {
            heap_used_diff: b.heap_used as i64 - a.heap_used as i64,
            processes_diff: b.running_processes as i32 - a.running_processes as i32,
            files_diff: b.open_files as i32 - a.open_files as i32,
            tick_elapsed_ns: b.tick_ns.saturating_sub(a.tick_ns),
        }
    }
}
