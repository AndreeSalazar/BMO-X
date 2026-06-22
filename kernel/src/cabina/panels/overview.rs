//! `cabina::panels::overview` — Panel Overview (resumen general).
//!
//! Muestra una vista compacta de TODAS las categorías para que el
//! usuario pueda ver el estado general del sistema de un vistazo.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::telemetry;

pub fn render(s: &Snapshot) {
    draw_header();
    let mut y = 40u32;

    draw_section_title(&mut y, "System");
    draw_kv(&mut y, "Uptime",     &alloc::format!("{} ms ({} s)", s.uptime_ns / 1_000_000, s.uptime_ns / 1_000_000_000), 0xFF00FF00);
    draw_kv(&mut y, "Boot phase", "Welcome", 0xFFCCCCCC);
    draw_kv(&mut y, "Ready",     if crate::cabina::is_ready() { "yes" } else { "no" }, if crate::cabina::is_ready() { 0xFF00FF00 } else { 0xFFFF8800 });

    draw_section_title(&mut y, "CPU");
    draw_kv(&mut y, "Interrupts", &alloc::format!("{}", s.cpu.interrupts), 0xFFFFFFFF);
    draw_kv(&mut y, "Faults PF",  &alloc::format!("{}", s.cpu.pf),  0xFFFFFF00);
    draw_kv(&mut y, "Faults GP",  &alloc::format!("{}", s.cpu.gp),  0xFFFF8800);
    draw_kv(&mut y, "Faults DF",  &alloc::format!("{}", s.cpu.df),  0xFFFF0000);

    draw_section_title(&mut y, "Memory");
    draw_kv(&mut y, "Heap used",  &alloc::format!("{} KB", s.memory.heap_used / 1024), 0xFF00FF00);
    draw_kv(&mut y, "Heap peak",  &alloc::format!("{} KB", s.memory.heap_peak / 1024), 0xFFFFFF00);
    draw_kv(&mut y, "Allocs",     &alloc::format!("{}", s.memory.allocs), 0xFFFFFFFF);
    draw_kv(&mut y, "Live",       &alloc::format!("{}", s.memory.allocs.saturating_sub(s.memory.frees)), 0xFF00FFFF);
    draw_kv(&mut y, "Free pages", &alloc::format!("{}", s.memory.free_pages), 0xFFCCCCCC);

    draw_section_title(&mut y, "Scheduler");
    draw_kv(&mut y, "Ctx switches", &alloc::format!("{}", s.scheduler.ctx_switches), 0xFF00FF00);
    draw_kv(&mut y, "Processes",    &alloc::format!("{}", s.scheduler.processes), 0xFFFFFFFF);
    draw_kv(&mut y, "Threads",      &alloc::format!("{}", s.scheduler.threads), 0xFFCCCCCC);

    draw_section_title(&mut y, "I/O");
    draw_kv(&mut y, "Serial bytes",  &alloc::format!("{}", s.io.serial_bytes), 0xFF00FF00);
    draw_kv(&mut y, "PCI reads",     &alloc::format!("{}", s.io.pci_reads),    0xFFCCCCCC);
    draw_kv(&mut y, "PCI writes",    &alloc::format!("{}", s.io.pci_writes),   0xFFCCCCCC);
    draw_kv(&mut y, "PS/2 scancodes", &alloc::format!("{}", s.io.ps2_scans),     0xFFCCCCCC);

    draw_section_title(&mut y, "Syscalls");
    draw_kv(&mut y, "Total",         &alloc::format!("{}", telemetry::syscall::get_total()), 0xFF00FF00);
    draw_kv(&mut y, "Unique active", &alloc::format!("{}", s.syscalls.len()), 0xFFFFFFFF);

    draw_section_title(&mut y, "GPU");
    draw_kv(&mut y, "Status", "Not yet (RDNA4 in v1.9)", 0xFFFF8800);

    draw_section_title(&mut y, "Last events");
    let mut i = 0;
    for ev in s.last_events.iter().take(8) {
        let val = if ev.value != 0 { alloc::format!(" (0x{:x})", ev.value) } else { alloc::string::String::new() };
        draw_kv(&mut y,
                 &alloc::format!("#{} {}", ev.seq, ev.severity.name()),
                 &alloc::format!("{}: {}{}", ev.module, ev.msg, val),
                 ev.severity.color());
        i += 1;
    }
    if i == 0 {
        draw_kv(&mut y, "", "(no events)", 0xFF888888);
    }
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF202028);
    draw_text(8, 8, "OVERVIEW", 0xFFCCCCCC);
    draw_text(100, 8, "— System summary", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF202028);
    draw_text(8, *y + 2, title, 0xFF00FF00);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.overview] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
