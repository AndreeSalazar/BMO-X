//! `cabina::panels::query` — Panel del DSL de queries.

#![allow(dead_code)]

use crate::cabina::panels::helpers as H;
use crate::cabina::paint;
use crate::cabina::query;

pub fn render(_s: &crate::cabina::snapshot::Snapshot) {
    H::header("QUERY", 0xFF44FF44);

    let mut y = 40u32;

    y = H::section(y, "DSL examples", 0xFF44FF44);
    let examples = [
        "errores", "critico", "capa:bmo_core", "modulo:lang", "valor>0x1000",
        "severidad:Panic", "ultimo:1s", "nuevos", "repetidos",
        "memoria:creciendo", "syscalls:lentas",
    ];
    for e in &examples {
        let line = alloc::format!("query \"{}\"", e);
        paint::draw_text(16, y, &line, 0xFF00FFFF);
        y += 14;
    }

    y = H::section(y, "Presets (5 smart)", 0xFF44FF44);
    let presets = [
        ("only_errors",     "0"),
        ("only_critical",   "1"),
        ("layer",           "2"),
        ("process",         "3"),
        ("thread",          "4"),
        ("syscall",         "5"),
        ("file",            "6"),
        ("before_panic",    "7"),
        ("only_new",        "8"),
        ("only_repeated",   "9"),
        ("memory_growing",  "10"),
        ("slow_syscalls",   "11"),
    ];
    for (k, v) in &presets {
        y = H::kv(y, k, v, 0xFF00FFAA);
    }

    y = H::section(y, "QueryId (F8)", 0xFF44FF44);
    y = H::kv(y, "0", "OnlyErrors",   0xFFFF0000);
    y = H::kv(y, "1", "OnlyCritical", 0xFFFF8800);
    y = H::kv(y, "2", "Kernel",       0xFFFFFF00);
    y = H::kv(y, "3", "Ring3",        0xFF00FF00);
    y = H::kv(y, "4", "Gpu",          0xFF00FFFF);
    y = H::kv(y, "5", "All",          0xFFCCCCCC);

    y = H::section(y, "Live test", 0xFF44FF44);
    let input = "errores";
    paint::draw_text(16, y, "input:", 0xFFCCCCCC);
    paint::draw_text(80, y, input, 0xFFFFFF00);
    y += 16;
    match query::parse(input) {
        Some(q) => {
            let desc = alloc::format!("-> Query {{ entities: {:?}, sev: {:?} }}",
                                        q.entities, q.severities);
            paint::draw_text(16, y, &desc, 0xFF00FF00);
        }
        None => {
            paint::draw_text(16, y, "-> None (parse failed)", 0xFFFF0000);
        }
    }
    y += 16;
    let _ = y;
}
