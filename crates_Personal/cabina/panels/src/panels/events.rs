use crate::fb::{self, FrameBuffer};
use crate::panels::helpers as H;
use crate::query::Query;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, s: &SystemSnapshot) {
    H::header(fb, "EVENTS", 0xFFFF00FF);
    let mut y = 40u32;

    let filter = Query::only_critical();
    let count = s.events[..s.event_count as usize].iter().filter(|e| filter.matches(e)).count();
    let title = alloc::format!("Filter: only_critical ({} events match)", count);
    y = H::section(fb, y, &title, 0xFFFF00FF);
    y = H::kv(fb, y, "F1", "only Info", 0xFFCCCCCC);
    y = H::kv(fb, y, "F2", "only Warning", 0xFFFFAA00);
    y = H::kv(fb, y, "F3", "only Fault", 0xFFFF8800);
    y = H::kv(fb, y, "F4", "only Panic", 0xFFFF0000);
    y = H::kv(fb, y, "F5", "all events", 0xFFCCCCCC);

    y = H::section(fb, y, "Events", 0xFFFF00FF);
    fb::draw_text(fb, 16, y, "Seq", 0xFFCCCCCC);
    fb::draw_text(fb, 80, y, "Sev", 0xFFCCCCCC);
    fb::draw_text(fb, 180, y, "Module", 0xFFCCCCCC);
    fb::draw_text(fb, 320, y, "Message", 0xFFCCCCCC);
    y += 16;
    let mut i = 0;
    for ev in s.events[..s.event_count as usize].iter() {
        if !filter.matches(ev) {
            continue;
        }
        let line = if ev.value != 0 {
            alloc::format!("#{} {:<9} {:<10} {} (0x{:x})", ev.seq, ev.severity.name(), ev.module_str(), ev.msg_str(), ev.value)
        } else {
            alloc::format!("#{} {:<9} {:<10} {}", ev.seq, ev.severity.name(), ev.module_str(), ev.msg_str())
        };
        fb::draw_text(fb, 16, y, &line, ev.severity.color());
        y += 14;
        i += 1;
        if i >= 30 {
            break;
        }
    }
    if i == 0 {
        fb::draw_text(fb, 16, y, "(no matching events)", 0xFF888888);
    }
}
