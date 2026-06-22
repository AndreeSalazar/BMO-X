//! `cabina::panels::events` — Panel de eventos con filtros.
//!
//! Muestra los últimos N eventos del log, con filtros aplicables:
//! - Por severidad (Info/Trace/Warning/Fault/Panic)
//! - Por módulo (fs, lang, kbc, BMO, ...)
//! - Por texto (substring search)
//!
//! v1.8.8: muestra todos los eventos. Los filtros son de cara al futuro
//! (UI con F-keys para cambiar filtro).

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::filter::EventFilter;
use crate::cabina::event::Severity;

pub fn render(s: &Snapshot) {
    draw_header();
    let mut y = 40u32;

    // Filtro por defecto: solo críticos.
    let filter = EventFilter::only_critical();

    draw_section_title(&mut y, &alloc::format!("Filter: only_critical ({} events match)", {
        let mut c = 0;
        for e in &s.last_events { if filter.matches(e) { c += 1; } }
        c
    }));

    draw_kv(&mut y, "F1",   "only Info",    0xFFCCCCCC);
    draw_kv(&mut y, "F2",   "only Warning", 0xFFFFAA00);
    draw_kv(&mut y, "F3",   "only Fault",   0xFFFF8800);
    draw_kv(&mut y, "F4",   "only Panic",   0xFFFF0000);
    draw_kv(&mut y, "F5",   "all events",   0xFFCCCCCC);
    draw_kv(&mut y, "F6",   "by module",    0xFF00FFFF);
    draw_kv(&mut y, "F7",   "search",       0xFF00FF00);

    draw_section_title(&mut y, "Events");
    draw_kv(&mut y, "Seq",   "Severity  Module     Message", 0xFFCCCCCC);
    let mut i = 0;
    for ev in &s.last_events {
        if !filter.matches(ev) { continue; }
        let val = if ev.value != 0 { alloc::format!(" (0x{:x})", ev.value) } else { alloc::string::String::new() };
        draw_kv(&mut y,
                 &alloc::format!("#{}", ev.seq),
                 &alloc::format!("{:<9} {:<10} {}{}", ev.severity.name(), ev.module, ev.msg, val),
                 ev.severity.color());
        i += 1;
        if i >= 30 { break; } // max 30 eventos
    }
    if i == 0 {
        draw_kv(&mut y, "", "(no matching events)", 0xFF888888);
    }
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF2E1A2E);
    draw_text(8, 8, "EVENTS", 0xFFFF00FF);
    draw_text(80, 8, "— Filtered event log", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF282028);
    draw_text(8, *y + 2, title, 0xFFFF00FF);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.events] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
