//! `timeback::snapshot` — Captura del estado del sistema en un instante.

#![allow(dead_code)]

/// Snapshot del sistema. v1.8.8: stub.
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    pub epoch: u64,
    pub tick_ns: u64,
    pub heap_used: u64,
    pub running_processes: u32,
    pub open_files: u32,
}

impl Snapshot {
    pub fn capture() -> Self {
        Self {
            epoch: super::current_epoch(),
            tick_ns: 0,
            heap_used: 0,
            running_processes: 0,
            open_files: 0,
        }
    }
}
