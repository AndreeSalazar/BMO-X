//! `cabina::panels::sched` — Panel de scheduler con detalle granular.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::panels::helpers as H;
use crate::cabina::paint;

pub fn render(s: &Snapshot) {
    H::header("SCHED", 0xFFFFFF00);

    let mut y = 40u32;
    let sc = &s.scheduler;

    y = H::section(y, "Global", 0xFFFFFF00);
    y = H::kv_u64(y, "Context switches", sc.ctx_switches, 0xFF00FF00);
    y = H::kv_u64(y, "Processes",        sc.processes, 0xFFFFFFFF);
    y = H::kv_u64(y, "Threads",          sc.threads, 0xFFCCCCCC);

    y = H::section(y, "Processes (v1.9: per-proc)", 0xFFFFFF00);
    let procs = [
        ("PID 0  init",      "running",  0xFF00FF00),
        ("PID 1  bmo_core",  "running",  0xFF00FF00),
        ("PID 2  desktop",   "running",  0xFF00FF00),
        ("PID 3  ring3",     "(none)",   0xFF888888),
    ];
    for (k, v, c) in &procs {
        y = H::kv(y, k, v, *c);
    }

    y = H::section(y, "Run queues", 0xFFFFFF00);
    y = H::kv(y, "Ready",    "0 (v1.9)", 0xFF888888);
    y = H::kv(y, "Blocked",  "0 (v1.9)", 0xFF888888);
    y = H::kv(y, "Sleeping", "0 (v1.9)", 0xFF888888);

    y = H::section(y, "Time slices (v1.9)", 0xFFFFFF00);
    y = H::kv(y, "Quantum",         "10 ms", 0xFFCCCCCC);
    y = H::kv(y, "Used (current)",  "0",     0xFFCCCCCC);
    let _ = y;
    let _ = paint::fill_rect;
}
