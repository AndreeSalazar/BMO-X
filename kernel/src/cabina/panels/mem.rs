//! `cabina::panels::mem` — Panel de memoria con detalle granular.
//!
//! Categorías:
//! - **Heap**: usado, pico, fragmentación, allocs/frees
//! - **Páginas**: libres, usadas, totales
//! - **Por tamaño**: allocs por bucket (8, 16, 32, 64, ..., 4096+)
//! - **Layout**: mapa de memoria (low memory, kernel heap, user, etc.)

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;

pub fn render(s: &Snapshot) {
    draw_header();
    let mut y = 40u32;
    let m = &s.memory;

    draw_section_title(&mut y, "Heap");
    draw_kv(&mut y, "Used",        &alloc::format!("{} B ({} KB)", m.heap_used, m.heap_used / 1024), 0xFF00FF00);
    draw_kv(&mut y, "Peak",        &alloc::format!("{} B", m.heap_peak), 0xFFFFFF00);
    draw_kv(&mut y, "Allocs",      &alloc::format!("{}", m.allocs), 0xFFFFFFFF);
    draw_kv(&mut y, "Frees",       &alloc::format!("{}", m.frees), 0xFFCCCCCC);
    draw_kv(&mut y, "Live",        &alloc::format!("{}", m.allocs.saturating_sub(m.frees)), 0xFF00FFFF);
    draw_kv(&mut y, "Fragment.",   "0% (bump allocator v1.8.8)", 0xFF888888);

    draw_section_title(&mut y, "Pages");
    draw_kv(&mut y, "Free",        &alloc::format!("{}", m.free_pages), 0xFF00FF00);
    draw_kv(&mut y, "Free KB",     &alloc::format!("{} KB", m.free_pages * 4), 0xFFCCCCCC);
    draw_kv(&mut y, "Used",        "(v1.9: total - free)", 0xFF888888);

    draw_section_title(&mut y, "Alloc buckets (v1.9)");
    draw_kv(&mut y, "8 B",         "0", 0xFF888888);
    draw_kv(&mut y, "16 B",        "0", 0xFF888888);
    draw_kv(&mut y, "32 B",        "0", 0xFF888888);
    draw_kv(&mut y, "64 B",        "0", 0xFF888888);
    draw_kv(&mut y, "128 B",       "0", 0xFF888888);
    draw_kv(&mut y, "256 B",       "0", 0xFF888888);
    draw_kv(&mut y, "512 B",       "0", 0xFF888888);
    draw_kv(&mut y, "1 KB",        "0", 0xFF888888);
    draw_kv(&mut y, "4 KB+",       "0", 0xFF888888);

    draw_section_title(&mut y, "Layout (v1.9)");
    draw_kv(&mut y, "0x0000_0000", "Reserved (low 1MB)", 0xFFCCCCCC);
    draw_kv(&mut y, "0x0010_0000", "Kernel text/data/bss", 0xFF00FF00);
    draw_kv(&mut y, "0x0100_0000", "Kernel heap",          0xFFFFFF00);
    draw_kv(&mut y, "0x1000_0000", "Framebuffer GOP",      0xFF00FFFF);
    draw_kv(&mut y, "0x4000_0000", "User (Ring 3)",        0xFFCCCCCC);
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF1A2E1A);
    draw_text(8, 8, "MEM", 0xFFAAFF00);
    draw_text(80, 8, "— Heap + Pages", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF202820);
    draw_text(8, *y + 2, title, 0xFFAAFF00);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.mem] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
