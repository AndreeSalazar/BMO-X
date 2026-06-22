//! `cabina::panels::sched` — Panel de scheduler con detalle granular.
//!
//! Categorías:
//! - **Global**: ctx switches, processes, threads
//! - **Por proceso**: PID, estado, threads, tiempo CPU
//! - **Run queues**: ready, blocked, sleeping
//! - **Time slices**: quantum usado, ticks restantes

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;

pub fn render(s: &Snapshot) {
    draw_header();
    let mut y = 40u32;
    let sc = &s.scheduler;

    draw_section_title(&mut y, "Global");
    draw_kv(&mut y, "Context switches", &alloc::format!("{}", sc.ctx_switches), 0xFF00FF00);
    draw_kv(&mut y, "Processes",        &alloc::format!("{}", sc.processes), 0xFFFFFFFF);
    draw_kv(&mut y, "Threads",          &alloc::format!("{}", sc.threads), 0xFFCCCCCC);

    draw_section_title(&mut y, "Processes (v1.9: per-proc)");
    draw_kv(&mut y, "PID 0  init",     "running",  0xFF00FF00);
    draw_kv(&mut y, "PID 1  bmo_core", "running",  0xFF00FF00);
    draw_kv(&mut y, "PID 2  desktop",  "running",  0xFF00FF00);
    draw_kv(&mut y, "PID 3  ring3",    "(none)",   0xFF888888);

    draw_section_title(&mut y, "Run queues");
    draw_kv(&mut y, "Ready",   "0 (v1.9)", 0xFF888888);
    draw_kv(&mut y, "Blocked", "0 (v1.9)", 0xFF888888);
    draw_kv(&mut y, "Sleeping","0 (v1.9)", 0xFF888888);

    draw_section_title(&mut y, "Time slices (v1.9)");
    draw_kv(&mut y, "Quantum", "10 ms", 0xFFCCCCCC);
    draw_kv(&mut y, "Used (current)", "0", 0xFFCCCCCC);
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF2E2E1A);
    draw_text(8, 8, "SCHED", 0xFFFFFF00);
    draw_text(80, 8, "— Process scheduler", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF282820);
    draw_text(8, *y + 2, title, 0xFFFFFF00);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.sched] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
