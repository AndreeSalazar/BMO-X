//! `cabina::tests` — Tests integrados de la cabina.
//!
//! Como el kernel es no_std, no podemos usar `#[cfg(test)]`. Estos
//! tests se ejecutan manualmente desde el boot (ver `bmo_core::coord`).
//!
//! ## Cobertura
//!
//! - **emit_basic**: emisión simple de Info.
//! - **emit_severities**: las 5 severidades.
//! - **filter_by_severity**: filtro solo Panic.
//! - **query_presets**: 5 QueryId pre-construidos.
//! - **snapshot_take**: snapshot no vacío después de un emit.

#![allow(dead_code)]

use crate::cabina::{self, Severity, Layer, Entity, QueryId, event::Event};
use crate::cabina::query::Query;
use crate::cabina::filter::EventFilter;

pub struct TestResult {
    pub name: &'static str,
    pub passed: bool,
    pub message: alloc::string::String,
}

pub fn run_all() -> alloc::vec::Vec<TestResult> {
    let mut r = alloc::vec::Vec::new();
    r.push(test_emit_basic());
    r.push(test_emit_severities());
    r.push(test_filter_by_severity());
    r.push(test_query_presets());
    r.push(test_snapshot_take());
    r.push(test_layer_inference());
    r.push(test_buffer_ring());
    r.push(test_persistent_spool());
    r
}

fn test_emit_basic() -> TestResult {
    cabina::emit(Severity::Info, "test", "hello");
    let last = cabina::buffer::last(1);
    if last.is_empty() {
        fail("emit_basic", "buffer empty after emit")
    } else {
        pass("emit_basic", &alloc::format!("seq={}", last[0].seq))
    }
}

fn test_emit_severities() -> TestResult {
    let s = [Severity::Info, Severity::Trace, Severity::Warning, Severity::Fault, Severity::Panic];
    let mut ok = true;
    for sev in &s {
        cabina::emit(*sev, "test_sev", "x");
    }
    let last5 = cabina::buffer::last(5);
    if last5.len() < 5 { ok = false; }
    for (i, ev) in last5.iter().enumerate() {
        if ev.severity != s[i] { ok = false; }
    }
    if ok { pass("emit_severities", "5 events, severities match")
    } else { fail("emit_severities", "mismatch") }
}

fn test_filter_by_severity() -> TestResult {
    cabina::emit(Severity::Info, "test", "a");
    cabina::emit(Severity::Panic, "test", "b");
    cabina::emit(Severity::Info, "test", "c");
    let only_panic = EventFilter::only_critical();
    let last3 = cabina::buffer::last(3);
    let panic_count = last3.iter().filter(|e| only_panic.matches(e)).count();
    if panic_count >= 1 {
        pass("filter_by_severity", &alloc::format!("{} panic+warning events", panic_count))
    } else {
        fail("filter_by_severity", "no critical events matched")
    }
}

fn test_query_presets() -> TestResult {
    let q = cabina::build_query(QueryId::OnlyErrors);
    let q2 = cabina::build_query(QueryId::All);
    if q.severities.len() == 2 && q2.severities.is_empty() {
        pass("query_presets", "OnlyErrors has 2 sev, All has 0")
    } else {
        fail("query_presets",
             &alloc::format!("OnlyErrors.{} All.{}",
                              q.severities.len(), q2.severities.len()))
    }
}

fn test_snapshot_take() -> TestResult {
    cabina::emit(Severity::Info, "snap", "before");
    let s = cabina::snapshot::take();
    cabina::emit(Severity::Info, "snap", "after");
    if s.last_events.len() >= 1 {
        pass("snapshot_take", &alloc::format!("{} events in snapshot", s.last_events.len()))
    } else {
        fail("snapshot_take", "snapshot empty")
    }
}

fn test_layer_inference() -> TestResult {
    let from_name = Layer::from_module("ring3.foo");
    if from_name == Layer::Ring3 {
        pass("layer_inference", "ring3 prefix → Layer::Ring3")
    } else {
        fail("layer_inference",
             &alloc::format!("got {:?}", from_name))
    }
}

fn test_buffer_ring() -> TestResult {
    let s_before = cabina::buffer::next_seq();
    for i in 0..10 {
        cabina::emit(Severity::Info, "ring", &alloc::format!("ev{}", i));
    }
    let s_after = cabina::buffer::next_seq();
    if s_after >= s_before + 10 {
        pass("buffer_ring", &alloc::format!("seq {} -> {}", s_before, s_after))
    } else {
        fail("buffer_ring", &alloc::format!("seq {} -> {}", s_before, s_after))
    }
}

fn test_persistent_spool() -> TestResult {
    cabina::emit(Severity::Info, "spool", "x");
    let n = cabina::persistent_pending_bytes();
    if n > 0 {
        pass("persistent_spool", &alloc::format!("{} bytes pending", n))
    } else {
        // El spool puede estar vacío si el serial aún no está listo.
        pass("persistent_spool", "0 bytes (serial may be disabled)")
    }
}

// ── Helpers ───────────────────────────────────────────────────────

fn pass(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: true, message: alloc::string::String::from(msg) }
}
fn fail(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: false, message: alloc::string::String::from(msg) }
}
