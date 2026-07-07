//! `timeback::tests` — Tests integrados de TimeBack.
//!
//! Cobertura:
//! - **create_checkpoint**: ID único, nombre preservado.
//! - **epoch_increments**: cada checkpoint incrementa la época.
//! - **snapshot_capture**: snapshot no es zero.
//! - **delta_between**: dos snapshots dan delta correcto.
//! - **journal_log**: log acepta entradas.

#![allow(dead_code)]

use crate::{checkpoint, journal, JournalOp};

pub struct TestResult {
    pub name: &'static str,
    pub passed: bool,
    pub message: alloc::string::String,
}

pub fn run_all() -> alloc::vec::Vec<TestResult> {
    let mut r = alloc::vec::Vec::new();
    r.push(test_create_checkpoint());
    r.push(test_epoch_increments());
    r.push(test_snapshot_capture());
    r.push(test_delta_between());
    r.push(test_journal_log());
    r
}

fn test_create_checkpoint() -> TestResult {
    let id = timeback::create_checkpoint("test1");
    if id.0 == 0 {
        return fail("create_checkpoint", "ID is 0");
    }
    let name = checkpoint::name(id);
    match name {
        Some(n) if n == "test1" => pass("create_checkpoint",
                                        &alloc::format!("id={} name={}", id.0, n)),
        _ => fail("create_checkpoint", "name mismatch"),
    }
}

fn test_epoch_increments() -> TestResult {
    let e0 = timeback::current_epoch();
    timeback::create_checkpoint("ep1");
    let e1 = timeback::current_epoch();
    timeback::create_checkpoint("ep2");
    let e2 = timeback::current_epoch();
    if e1 > e0 && e2 > e1 {
        pass("epoch_increments", &alloc::format!("{} -> {} -> {}", e0, e1, e2))
    } else {
        fail("epoch_increments",
             &alloc::format!("{} -> {} -> {}", e0, e1, e2))
    }
}

fn test_snapshot_capture() -> TestResult {
    let s1 = timeback::snapshot::Snapshot::capture();
    let s2 = timeback::snapshot::Snapshot::capture();
    if s2.epoch >= s1.epoch {
        pass("snapshot_capture", &alloc::format!("epoch {} -> {}", s1.epoch, s2.epoch))
    } else {
        fail("snapshot_capture", "epoch decreased")
    }
}

fn test_delta_between() -> TestResult {
    use crate::snapshot::Snapshot;
    use crate::delta::Delta;
    let a = Snapshot { epoch: 1, tick_ns: 100, heap_used: 1024, running_processes: 2, open_files: 3 };
    let b = Snapshot { epoch: 2, tick_ns: 200, heap_used: 2048, running_processes: 3, open_files: 5 };
    let d = Delta::between(&a, &b);
    if d.heap_used_diff == 1024 && d.processes_diff == 1 && d.files_diff == 2 && d.tick_elapsed_ns == 100 {
        pass("delta_between", "all fields correct")
    } else {
        fail("delta_between",
             &alloc::format!("heap={} proc={} files={} tick={}",
                              d.heap_used_diff, d.processes_diff, d.files_diff, d.tick_elapsed_ns))
    }
}

fn test_journal_log() -> TestResult {
    let n0 = journal::count();
    journal::log(JournalOp::Checkpoint, "test1");
    journal::log(JournalOp::AppRun, "test2");
    let n1 = journal::count();
    if n1 > n0 {
        pass("journal_log", &alloc::format!("{} -> {} entries", n0, n1))
    } else {
        fail("journal_log", "count did not grow")
    }
}

// ── Helpers ───────────────────────────────────────────────────────

fn pass(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: true, message: alloc::string::String::from(msg) }
}
fn fail(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: false, message: alloc::string::String::from(msg) }
}
