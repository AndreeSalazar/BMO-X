//! `timeback::journal` — Log de operaciones (ring buffer).

extern crate alloc;

#![allow(dead_code)]

use alloc::string::String;

/// Tipo de operación registrada en el journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalOp {
    /// App ejecutada.
    AppRun,
    /// App finalizada.
    AppExit,
    /// Archivo creado.
    FileCreate,
    /// Archivo modificado.
    FileWrite,
    /// Archivo eliminado.
    FileDelete,
    /// Checkpoint creado.
    Checkpoint,
    /// Rollback ejecutado.
    Rollback,
}

/// Entrada del journal.
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
static mut BUF: [Option<JournalEntry>; MAX_JOURNAL] = [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None];

pub fn init() {}

/// Registra una entrada en el journal.
pub fn log(op: JournalOp, detail: &str) {
    unsafe {
        let seq = (HEAD as u64) + 1;
        BUF[HEAD] = Some(JournalEntry {
            seq,
            epoch: super::current_epoch(),
            op,
            detail: String::from(detail),
        });
        HEAD = (HEAD + 1) % MAX_JOURNAL;
        if COUNT < MAX_JOURNAL { COUNT += 1; }
    }
}

/// # de entradas en el journal.
pub fn count() -> usize { unsafe { COUNT } }
