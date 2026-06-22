//! `cabina::panels::query` — Panel de query con filtros y colores.
//!
//! Muestra el query activo (F8 para ciclar) y los resultados
//! filtrados con colores por capa y severidad.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::event::{Layer, Severity};
use crate::cabina::{self, active_query, active_query_name};

pub fn render(s: &Snapshot) {
    draw_header();
    let mut y = 40u32;

    // ── Filtro activo ──
    draw_section_title(&mut y, "Active query");
    draw_kv(&mut y, "F8", "cycle query", 0xFFCCCCCC);
    draw_kv(&mut y, "Query", active_query_name(), 0xFF00FFFF);
    let q = active_query();
    let layers_str = format_layers(&q);
    draw_kv(&mut y, "Layers", &layers_str, 0xFFFFFFFF);
    let sev_str = format_severities(&q);
    draw_kv(&mut y, "Severities", &sev_str, 0xFFFFFF00);

    draw_section_title(&mut y, "Layer legend (F1..F8 = toggle)");
    draw_kv_color(&mut y, "ring0",    "hardware, CPU, IRQ",      Layer::Ring0.color());
    draw_kv_color(&mut y, "bmo_core", "windowing, FS, desktop",  Layer::BmoCore.color());
    draw_kv_color(&mut y, "bmo_gpu",  "RDNA4 (v1.9)",            Layer::BmoGpu.color());
    draw_kv_color(&mut y, "ring3",    "userland apps",           Layer::Ring3.color());
    draw_kv_color(&mut y, "lang",     "AOT, linker, parser",     Layer::Lang.color());
    draw_kv_color(&mut y, "fs",       "filesystem",              Layer::Fs.color());
    draw_kv_color(&mut y, "net",      "TCP/UDP (v1.9)",          Layer::Net.color());
    draw_kv_color(&mut y, "sec",      "capabilities",            Layer::Sec.color());

    draw_section_title(&mut y, "Severity legend");
    draw_kv_color(&mut y, "INFO",    "boot/operación normal",   Severity::Info.color());
    draw_kv_color(&mut y, "TRACE",   "debugging fino",          Severity::Trace.color());
    draw_kv_color(&mut y, "WARN",    "advertencia",             Severity::Warning.color());
    draw_kv_color(&mut y, "FAULT",   "error recuperable",       Severity::Fault.color());
    draw_kv_color(&mut y, "PANIC",   "no recuperable",          Severity::Panic.color());

    draw_section_title(&mut y, "Query results");
    let filtered = q.apply(&s.last_events);
    let mut i = 0;
    for ev in &filtered {
        let val = if ev.value != 0 { alloc::format!(" (0x{:x})", ev.value) } else { alloc::string::String::new() };
        let eid = if ev.entity_id != 0 {
            alloc::format!("[{}#{}]", ev.entity.name(), ev.entity_id)
        } else { alloc::string::String::new() };
        // Color = layer (fondo conceptual) + severity (foreground).
        let color = ev.severity.color();
        draw_event(&mut y, ev.seq, &ev.layer.name(), ev.severity.name(),
                    &ev.module, &ev.msg, &val, &eid, color, ev.layer.color());
        i += 1;
        if i >= 30 { break; }
    }
    if i == 0 {
        draw_kv(&mut y, "", "(no matching events)", 0xFF888888);
    }

    draw_section_title(&mut y, "Quick queries");
    draw_kv(&mut y, "F1", "only Errors (Fault+Panic)", 0xFFFF8800);
    draw_kv(&mut y, "F2", "only Critical (Warn+Fault+Panic)", 0xFFFFFF00);
    draw_kv(&mut y, "F3", "kernel (Ring0 + BmoCore)", 0xFFFF4444);
    draw_kv(&mut y, "F4", "Ring3 only", 0xFF44FF44);
    draw_kv(&mut y, "F5", "BmoGpu only", 0xFF00FFFF);
    draw_kv(&mut y, "F6", "all events", 0xFFCCCCCC);
    draw_kv(&mut y, "F7", "search", 0xFFAAFF00);
}

fn format_layers(q: &crate::cabina::query::Query) -> alloc::string::String {
    if q.layers.is_empty() { return alloc::string::String::from("(all)"); }
    let mut s = alloc::string::String::new();
    for (i, l) in q.layers.iter().enumerate() {
        if i > 0 { s.push_str(", "); }
        s.push_str(l.name());
    }
    s
}

fn format_severities(q: &crate::cabina::query::Query) -> alloc::string::String {
    if q.severities.is_empty() { return alloc::string::String::from("(all)"); }
    let mut s = alloc::string::String::new();
    for (i, sv) in q.severities.iter().enumerate() {
        if i > 0 { s.push_str(", "); }
        s.push_str(sv.name());
    }
    s
}

fn draw_event(
    y: &mut u32,
    seq: u64,
    layer: &str,
    sev: &str,
    module: &str,
    msg: &str,
    val: &str,
    eid: &str,
    color: u32,
    _bg: u32,
) {
    let line = alloc::format!("#{} [{}|{}] {} {}{}{}",
        seq, layer, sev, module, msg, val, eid);
    draw_text(16, *y, &line, color);
    *y += 14;
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF2E1A2E);
    draw_text(8, 8, "QUERY", 0xFF00FFFF);
    draw_text(80, 8, "— Smart filter (F8 = cycle)", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF282028);
    draw_text(8, *y + 2, title, 0xFF00FFFF);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_kv_color(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, color);
    draw_text(280, *y, val, 0xFFCCCCCC);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.query] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
