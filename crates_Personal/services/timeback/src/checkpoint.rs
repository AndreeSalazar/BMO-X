//! `timeback::checkpoint` — Legacy checkpoint IDs (v1.8.x compat).
//!
//! New code should use `repo::commit()` instead.

extern crate alloc;

use alloc::string::String;
use core::mem::MaybeUninit;

use super::snapshot::Snapshot;
use super::storage;

const MAX_CHECKPOINTS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CheckpointId(pub u32);

static mut NEXT_ID: u32 = 1;
static mut COUNT: usize = 0;
static mut NAMES: [MaybeUninit<Option<String>>; MAX_CHECKPOINTS] = [const { MaybeUninit::uninit() }; MAX_CHECKPOINTS];

pub fn create(name: &str) -> CheckpointId {
    unsafe {
        let id = NEXT_ID;
        NEXT_ID += 1;
        let slot = (id as usize - 1) % MAX_CHECKPOINTS;
        NAMES[slot].write(Some(String::from(name)));
        if COUNT < MAX_CHECKPOINTS { COUNT += 1; }

        // Persist snapshot to NVRAM
        let snap = Snapshot::capture();
        let mut buf = [0u8; 64];
        let mut p = 0;
        buf[p..p+8].copy_from_slice(&snap.epoch.to_le_bytes()); p += 8;
        buf[p..p+8].copy_from_slice(&snap.tick_ns.to_le_bytes()); p += 8;
        buf[p..p+8].copy_from_slice(&snap.heap_used.to_le_bytes()); p += 8;
        buf[p..p+4].copy_from_slice(&snap.running_processes.to_le_bytes()); p += 4;
        buf[p..p+4].copy_from_slice(&snap.open_files.to_le_bytes()); p += 4;
        storage::persist_to_nvram(id, &buf[..p]);

        CheckpointId(id)
    }
}

pub fn name(id: CheckpointId) -> Option<String> {
    unsafe {
        let slot = (id.0 as usize - 1) % MAX_CHECKPOINTS;
        NAMES[slot].assume_init_read().clone()
    }
}

pub fn count() -> usize { unsafe { COUNT } }
