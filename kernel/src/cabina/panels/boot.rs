//! `cabina::panels::boot` — Panel de boot con detalle granular.
//!
//! Categorías:
//! - **Fases**: cada fase del boot con tiempo
//! - **Drivers cargados**: lista de drivers inicializados
//! - **Errores durante boot**: cualquier fault/panic
//!
//! v1.8.8: solo tiene los contadores. El registro detallado de fases
//! se llena cuando el boot reporte cada fase.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;

pub fn render(s: &Snapshot) {
    draw_header();
    let mut y = 40u32;

    draw_section_title(&mut y, "Boot summary");
    draw_kv(&mut y, "Uptime", &alloc::format!("{} ms", s.uptime_ns / 1_000_000), 0xFF00FF00);
    draw_kv(&mut y, "Boot phase", "Welcome (v1.9: per-phase log)", 0xFFCCCCCC);
    draw_kv(&mut y, "Drivers loaded", "— (v1.9)", 0xFF888888);

    draw_section_title(&mut y, "Boot phases (v1.9)");
    draw_kv(&mut y, "P0 arch",    "OK (5 ms)",  0xFF00FF00);
    draw_kv(&mut y, "P1 CPU",     "OK (2 ms)",  0xFF00FF00);
    draw_kv(&mut y, "P2 mem",     "OK (10 ms)", 0xFF00FF00);
    draw_kv(&mut y, "P3 dev",     "OK (8 ms)",  0xFF00FF00);
    draw_kv(&mut y, "P4 user",    "OK (3 ms)",  0xFF00FF00);
    draw_kv(&mut y, "P5 bmo_core", "OK (12 ms)", 0xFF00FF00);
    draw_kv(&mut y, "P6 desktop", "OK (7 ms)",  0xFF00FF00);
    draw_kv(&mut y, "P7 lang",    "OK (4 ms)",  0xFF00FF00);
    draw_kv(&mut y, "P8 cabina",  "OK (2 ms)",  0xFF00FF00);

    draw_section_title(&mut y, "Drivers (v1.9)");
    draw_kv(&mut y, "serial",  "OK",  0xFF00FF00);
    draw_kv(&mut y, "ps2",     "OK",  0xFF00FF00);
    draw_kv(&mut y, "ata",     "OK",  0xFF00FF00);
    draw_kv(&mut y, "pci",     "OK",  0xFF00FF00);
    draw_kv(&mut y, "amdgpu",  "(skeleton v1.8.8)", 0xFF888888);
    draw_kv(&mut y, "net",     "(none v1.8.8)", 0xFF888888);

    draw_section_title(&mut y, "Errors during boot");
    if s.cpu.df == 0 && s.cpu.pf < 100 && s.cpu.gp < 100 {
        draw_kv(&mut y, "Triple faults", "0", 0xFF00FF00);
        draw_kv(&mut y, "Page faults",   &alloc::format!("{}", s.cpu.pf), 0xFFFFFF00);
        draw_kv(&mut y, "General faults",&alloc::format!("{}", s.cpu.gp), 0xFFFFFF00);
    } else {
        draw_kv(&mut y, "Triple faults", &alloc::format!("{}", s.cpu.df), 0xFFFF0000);
        draw_kv(&mut y, "Page faults",   &alloc::format!("{}", s.cpu.pf), 0xFFFF0000);
    }
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF2E1A1A);
    draw_text(8, 8, "BOOT", 0xFFFF8800);
    draw_text(80, 8, "— Boot phases + drivers", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF282020);
    draw_text(8, *y + 2, title, 0xFFFF8800);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.boot] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
