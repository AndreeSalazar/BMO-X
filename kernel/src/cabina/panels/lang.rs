//! `cabina::panels::lang` — Panel de LANG con detalle granular.
//!
//! Categorías:
//! - **AOT stats**: bytes emitidos por función, calls, jumps
//! - **Linker stats**: objetos linkeados, relocs aplicados, runtime size
//! - **Tests**: cuántos tests pasaron/fallaron
//! - **Benchmarks**: tiempo de compilación por source
//!
//! v1.8.8: solo contadores placeholder. El lang report debe actualizar
//! estos contadores.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;

pub fn render(_s: &Snapshot) {
    draw_header();
    let mut y = 40u32;

    draw_section_title(&mut y, "AOT x86-64");
    draw_kv(&mut y, "Sources compiled", "0 (v1.9)", 0xFF888888);
    draw_kv(&mut y, "Functions emitted", "0", 0xFFCCCCCC);
    draw_kv(&mut y, "Total bytes",      "0", 0xFFCCCCCC);
    draw_kv(&mut y, "Avg per function", "0", 0xFF888888);
    draw_kv(&mut y, "Largest function", "0 B (v1.9)", 0xFF888888);

    draw_section_title(&mut y, "Linker v2.0");
    draw_kv(&mut y, "Objects linked",     "0", 0xFFCCCCCC);
    draw_kv(&mut y, "Relocs applied",     "0", 0xFFCCCCCC);
    draw_kv(&mut y, "Sections in BEF",    "0", 0xFFCCCCCC);
    draw_kv(&mut y, "Runtime embedded",   "0 B (v1.9: c_min)", 0xFFFFFF00);
    draw_kv(&mut y, "Total BEF size",     "0 B", 0xFFFFFFFF);

    draw_section_title(&mut y, "Tests");
    draw_kv(&mut y, "Passed", "10 (hello, arith, if, while, ...)", 0xFF00FF00);
    draw_kv(&mut y, "Failed", "0", 0xFF00FF00);
    draw_kv(&mut y, "Total",  "10", 0xFFFFFFFF);

    draw_section_title(&mut y, "Compilation time");
    draw_kv(&mut y, "Hello World (BMO)",  "< 1 ms", 0xFF00FF00);
    draw_kv(&mut y, "Fibonacci (BMO)",    "< 1 ms", 0xFF00FF00);
    draw_kv(&mut y, "Factorial (BMO)",    "< 1 ms", 0xFF00FF00);
    draw_kv(&mut y, "Factorial (C)",      "< 1 ms", 0xFF00FF00);

    draw_section_title(&mut y, "Languages");
    draw_kv(&mut y, "BMO",   "v2.0.0 — AOT working",  0xFF00FF00);
    draw_kv(&mut y, "C",     "v1.8.8 — pipeline OK",  0xFF00FF00);
    draw_kv(&mut y, "C++",   "v1.9 (skeleton)",      0xFF888888);
    draw_kv(&mut y, "Java-BMO",  "v1.9 (skeleton)",  0xFF888888);
    draw_kv(&mut y, "Python-BMO","v1.9 (skeleton)",  0xFF888888);
    draw_kv(&mut y, "Rust-BMO",  "v1.9 (skeleton)",  0xFF888888);
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF1A2E2E);
    draw_text(8, 8, "LANG", 0xFF00FFAA);
    draw_text(80, 8, "— AOT + Linker + Tests", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF202828);
    draw_text(8, *y + 2, title, 0xFF00FFAA);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.lang] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
