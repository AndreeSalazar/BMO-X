//! `cabina::panels::cpu` — Panel de CPU con detalle granular.
//!
//! Categorías de datos:
//! - **Interrupts**: total, per-IRQ (0..23)
//! - **Faults**: PF, GP, NM, DF, UD, MC (con tasa por segundo)
//! - **TSC**: ticks, freq estimada
//! - **Timer**: ticks del PIT/HPET
//! - **CPU model**: vendor, family, model, stepping
//!
//! v1.8.8: la mayoría de los contadores son placeholders que se
//! llenarán cuando los drivers相应报告.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::telemetry;

/// Header del panel.
pub fn render(s: &Snapshot) {
    draw_header();
    let mut y = 40u32;
    let c = &s.cpu;

    // ── Sec 1: Interrupts ──
    draw_section_title(&mut y, "Interrupts");
    draw_kv(&mut y, "Total",   &alloc::format!("{}", c.interrupts), 0xFFFFFFFF);
    draw_kv(&mut y, "Timer",   &alloc::format!("{}", c.timer_ticks), 0xFFCCCCCC);
    draw_kv(&mut y, "Per-IRQ", "(v1.9: per_irq table)", 0xFF888888);

    // ── Sec 2: Faults ──
    draw_section_title(&mut y, "Faults");
    draw_kv(&mut y, "Page (#PF)",     &alloc::format!("{}", c.pf), 0xFFFFFF00);
    draw_kv(&mut y, "General (#GP)", &alloc::format!("{}", c.gp), 0xFFFF8800);
    draw_kv(&mut y, "NMI",            &alloc::format!("{}", c.nm), 0xFFFF4400);
    draw_kv(&mut y, "Double (#DF)",   &alloc::format!("{}", c.df), 0xFFFF0000);
    draw_kv(&mut y, "Invalid (#UD)",  &alloc::format!("{}", c.ud), 0xFFFF8800);
    draw_kv(&mut y, "Machine (#MC)",  &alloc::format!("{}", c.mc), 0xFFFF0000);

    // ── Sec 3: TSC + CPU info ──
    draw_section_title(&mut y, "TSC / CPU");
    draw_kv(&mut y, "TSC ticks",  &alloc::format!("{}", c.timer_ticks), 0xFFCCCCCC);
    draw_kv(&mut y, "TSC freq",   "5600X ~3.7 GHz", 0xFF00FF00);
    draw_kv(&mut y, "Vendor",     "AMD",            0xFFFFFFFF);
    draw_kv(&mut y, "Family",     "Zen 3 (0x19)",  0xFFCCCCCC);
    draw_kv(&mut y, "Model",      "0x21",           0xFFCCCCCC);
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF1A1A2E);
    draw_text(8, 8, "CPU", 0xFF00FFAA);
    draw_text(80, 8, "— AMD Ryzen 5 5600X (Zen 3)", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF202028);
    draw_text(8, *y + 2, title, 0xFF00FFAA);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.cpu] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
