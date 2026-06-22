//! `cabina::panels::gpu` — Panel de GPU (placeholder para RDNA4).
//!
//! v1.8.8: solo muestra que NO hay GPU activa todavía. Cuando se
//! implemente BMO GPU (RDNA4), este panel mostrará:
//! - device detection
//! - PCI bus/dev/fn + BARs
//! - VRAM total + libre
//! - gfx/compute/sdma ring status
//! - submitted command buffers
//! - GPU page faults
//! - IRQ count
//! - shader BSF loaded

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;

pub fn render(_s: &Snapshot) {
    draw_header();
    let mut y = 40u32;

    draw_section_title(&mut y, "GPU detection");
    draw_kv(&mut y, "Status",     "Not detected", 0xFFFF0000);
    draw_kv(&mut y, "Vendor",     "—",            0xFF888888);
    draw_kv(&mut y, "Device",     "—",            0xFF888888);
    draw_kv(&mut y, "Class",      "—",            0xFF888888);
    draw_kv(&mut y, "PCI bus",    "—",            0xFF888888);
    draw_kv(&mut y, "BAR0 size",  "—",            0xFF888888);
    draw_kv(&mut y, "BAR2 size",  "—",            0xFF888888);
    draw_kv(&mut y, "BAR5 size",  "—",            0xFF888888);

    draw_section_title(&mut y, "VRAM (v1.9)");
    draw_kv(&mut y, "Total",      "0 MB", 0xFF888888);
    draw_kv(&mut y, "Free",       "0 MB", 0xFF888888);
    draw_kv(&mut y, "Used",       "0 MB", 0xFF888888);
    draw_kv(&mut y, "GART size",  "0 MB", 0xFF888888);

    draw_section_title(&mut y, "Rings (v1.9)");
    draw_kv(&mut y, "GFX ring 0", "idle", 0xFF888888);
    draw_kv(&mut y, "GFX ring 1", "idle", 0xFF888888);
    draw_kv(&mut y, "Compute 0",  "idle", 0xFF888888);
    draw_kv(&mut y, "Compute 1",  "idle", 0xFF888888);
    draw_kv(&mut y, "SDMA 0",     "idle", 0xFF888888);
    draw_kv(&mut y, "SDMA 1",     "idle", 0xFF888888);

    draw_section_title(&mut y, "Stats (v1.9)");
    draw_kv(&mut y, "Submitted CBs",     "0", 0xFF888888);
    draw_kv(&mut y, "Completed CBs",     "0", 0xFF888888);
    draw_kv(&mut y, "GPU page faults",   "0", 0xFF888888);
    draw_kv(&mut y, "IRQ count",         "0", 0xFF888888);
    draw_kv(&mut y, "BSF shaders loaded", "0", 0xFF888888);
    draw_kv(&mut y, "Last fence",        "0", 0xFF888888);

    draw_section_title(&mut y, "Note");
    draw_kv(&mut y, "Roadmap",   "v1.9 (driver) / v2.0 (compute)", 0xFFFFFF00);
    draw_kv(&mut y, "Target",    "RX 9060 XT (RDNA4)",          0xFF00FFFF);
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF1A2E2E);
    draw_text(8, 8, "GPU", 0xFF00FFFF);
    draw_text(80, 8, "— BMO GPU (RDNA4) — placeholder", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF202828);
    draw_text(8, *y + 2, title, 0xFF00FFFF);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.gpu] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
