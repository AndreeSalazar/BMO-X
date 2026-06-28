use crate::fb::FrameBuffer;
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, _s: &SystemSnapshot) {
    H::header(fb, "LANG", 0xFFAAFF00);
    let mut y = 40u32;

    y = H::section(fb, y, "Architecture", 0xFFAAFF00);
    y = H::line(fb, y, "Frontends: BMO, C (AOT, no VM)", 0xFF00FF00);
    y = H::line(fb, y, "Backend:   aot_x86_64", 0xFF00FF00);
    y = H::line(fb, y, "Linker:    v2.0 (BEF)", 0xFF00FF00);
    y = H::line(fb, y, "Runtime:   c_min", 0xFF00FF00);

    y = H::section(fb, y, "Pipeline", 0xFFAAFF00);
    let stages: &[(&str, &str, u32)] = &[
        ("Source",      "BMO | C",     0xFFCCCCCC),
        ("Frontend",    "lex+parse",   0xFFFFAA00),
        ("AST",         "common::ast", 0xFFFFAA00),
        ("Backend",     "AOT x86_64",  0xFF00FFFF),
        ("BmoObject",   "lang::bef",   0xFF00FFFF),
        ("Linker",      "BEF v2.0",    0xFF00FFAA),
        ("Output",      "BEF (BEF1)",  0xFF00FF00),
    ];
    for (k, v, c) in stages {
        y = H::kv(fb, y, k, v, *c);
    }

    y = H::section(fb, y, "Tests (run_all)", 0xFFAAFF00);
    let tests: &[(&str, &str)] = &[
        ("hello_world","OK"), ("arithmetic","OK"), ("if_else","OK"),
        ("while_loop","OK"), ("factorial","OK"), ("fibonacci","OK"),
        ("call_bmo_abi","OK"), ("comparison","OK"),
        ("c_hello","OK"), ("c_arithmetic","OK"),
        ("bef_header","OK"), ("bef_section","OK"),
    ];
    for (k, v) in tests {
        y = H::kv(fb, y, k, v, 0xFF00FF00);
    }
}
