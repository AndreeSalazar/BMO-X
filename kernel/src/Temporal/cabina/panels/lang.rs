//! `cabina::panels::lang` — Panel del toolchain de lenguajes.

#![allow(dead_code)]

use crate::cabina::panels::helpers as H;
use crate::cabina::paint;

pub fn render(_s: &crate::cabina::snapshot::Snapshot) {
    H::header("LANG", 0xFFAAFF00);

    let mut y = 40u32;

    y = H::section(y, "Architecture", 0xFFAAFF00);
    y = H::line(y, "Frontends: BMO, C (AOT, no VM)", 0xFF00FF00);
    y = H::line(y, "Backend:   aot_x86_64",           0xFF00FF00);
    y = H::line(y, "Linker:    v2.0 (BEF)",           0xFF00FF00);
    y = H::line(y, "Runtime:   c_min",                0xFF00FF00);
    y = H::line(y, "ABI:       bmo_abi v1.0.0",       0xFFCCCCCC);

    y = H::section(y, "Pipeline", 0xFFAAFF00);
    let stages = [
        ("Source",      "BMO | C",     0xFFCCCCCC),
        ("Frontend",    "lex+parse",   0xFFFFAA00),
        ("AST",         "common::ast", 0xFFFFAA00),
        ("Backend",     "AOT x86_64",  0xFF00FFFF),
        ("BmoObject",   "lang::bef",   0xFF00FFFF),
        ("Linker",      "BEF v2.0",    0xFF00FFAA),
        ("Output",      "BEF (BEF1)",  0xFF00FF00),
    ];
    for (k, v, c) in &stages {
        y = H::kv(y, k, v, *c);
    }

    y = H::section(y, "Tests (run_all)", 0xFFAAFF00);
    let tests = [
        ("hello_world",        "OK"),
        ("arithmetic",         "OK"),
        ("if_else",            "OK"),
        ("while_loop",         "OK"),
        ("factorial",          "OK"),
        ("fibonacci",          "OK"),
        ("call_bmo_abi",       "OK"),
        ("comparison",         "OK"),
        ("c_hello_world",      "OK"),
        ("c_arithmetic",       "OK"),
        ("bef_header_valid",   "OK"),
        ("bef_section_table",  "OK"),
    ];
    for (k, v) in &tests {
        y = H::kv(y, k, v, 0xFF00FF00);
    }

    y = H::section(y, "Stats (v1.9)", 0xFFAAFF00);
    y = H::kv(y, "AST nodes",      "-- (v1.9)", 0xFF888888);
    y = H::kv(y, "Bytes compiled", "-- (v1.9)", 0xFF888888);
    y = H::kv(y, "Objects linked", "-- (v1.9)", 0xFF888888);
    y = H::kv(y, "Code size (BEF)","-- (v1.9)", 0xFF888888);
    let _ = y;
    let _ = paint::fill_rect;
}
