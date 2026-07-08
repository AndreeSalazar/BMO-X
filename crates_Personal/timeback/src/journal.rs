//! `timeback::journal` — Log de operaciones (ring buffer).
//!
//! v1.9: kept as a simple in-RAM ring buffer for transient state changes.
//! Persistent state is in the SSD repo (storage::write_object).

extern crate alloc;

use alloc::string::String;
use core::mem::MaybeUninit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalOp {
    AppRun,
    AppExit,
    FileCreate,
    FileWrite,
    FileDelete,
    Checkpoint,
    Rollback,
}

#[derive(Clone, Debug)]
pub struct JournalEntry {
    pub seq: u64,
    pub epoch: u64,
    pub op: JournalOp,
    pub detail: String,
}

const MAX_JOURNAL: usize = 256;

static mut HEAD: usize = 0;
static mut COUNT: usize = 0;
static mut BUF: [MaybeUninit<Option<JournalEntry>>; MAX_JOURNAL] = [const { MaybeUninit::uninit() }; MAX_JOURNAL];

pub fn init() {}

pub fn log(op: JournalOp, detail: &str) {
    unsafe {
        let seq = (HEAD as u64) + 1;
        BUF[HEAD].write(Some(JournalEntry {
            seq,
            epoch: super::current_epoch(),
            op,
            detail: String::from(detail),
        }));
        HEAD = (HEAD + 1) % MAX_JOURNAL;
        if COUNT < MAX_JOURNAL { COUNT += 1; }
    }
}

pub fn count() -> usize { unsafe { COUNT } }
