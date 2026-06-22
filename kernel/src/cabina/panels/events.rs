//! `cabina::panels::events` — Panel de eventos con filtros.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::filter::EventFilter;
use crate::cabina::event::Severity;
use crate::cabina::panels::helpers as H;
use crate::cabina::paint;

pub fn render(s: &Snapshot) {
    H::header("EVENTS", 0xFFFF00FF);

    let mut y = 40u32;

    let filter = EventFilter::only_critical();
    let mut count = 0;
    for e in &s.last_events { if filter.matches(e) { count += 1; } }
    let title = alloc::format!("Filter: only_critical ({} events match)", count);
    y = H::section(y, &title, 0xFFFF00FF);
    y = H::kv(y, "F1", "only Info", 0xFFCCCCCC);
    y = H::kv(y, "F2", "only Warning", 0xFFFFAA00);
    y = H::kv(y, "F3", "only Fault", 0xFFFF8800);
    y = H::kv(y, "F4", "only Panic", 0xFFFF0000);
    y = H::kv(y, "F5", "all events", 0xFFCCCCCC);
    y = H::kv(y, "F6", "by module", 0xFF00FFFF);
    y = H::kv(y, "F7", "search", 0xFF00FF00);

    y = H::section(y, "Events", 0xFFFF00FF);
    paint::draw_text(16, y, "Seq", 0xFFCCCCCC);
    paint::draw_text(80, y, "Sev", 0xFFCCCCCC);
    paint::draw_text(180, y, "Module", 0xFFCCCCCC);
    paint::draw_text(320, y, "Message", 0xFFCCCCCC);
    y += 16;
    let mut i = 0;
    for ev in &s.last_events {
        if !filter.matches(ev) { continue; }
        let line = if ev.value != 0 {
            alloc::format!("#{} {:<9} {:<10} {} (0x{:x})", ev.seq, ev.severity.name(), ev.module, ev.msg, ev.value)
        } else {
            alloc::format!("#{} {:<9} {:<10} {}", ev.seq, ev.severity.name(), ev.module, ev.msg)
        };
        let _ = (ev.severity == Severity::Panic);
        paint::draw_text(16, y, &line, ev.severity.color());
        y += 14;
        i += 1;
        if i >= 30 { break; }
    }
    if i == 0 {
        paint::draw_text(16, y, "(no matching events)", 0xFF888888);
    }
    let _ = y;
}
