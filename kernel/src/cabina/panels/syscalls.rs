//! `cabina::panels::syscalls` — Panel de syscalls con detalle completo.
//!
//! Muestra TODOS los syscalls que han sido invocados al menos una vez,
//! con su nombre, número, count, y categoría (windowing, fs, ipc, etc.).

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::telemetry::syscall;

pub fn render(s: &Snapshot) {
    draw_header();
    let mut y = 40u32;

    draw_section_title(&mut y, "Summary");
    draw_kv(&mut y, "Total syscalls", &alloc::format!("{}", syscall::get_total()), 0xFF00FF00);
    draw_kv(&mut y, "Unique called",  &alloc::format!("{}", s.syscalls.len()), 0xFFFFFFFF);

    draw_section_title(&mut y, "Per-syscall (sorted by count)");
    draw_kv(&mut y, "NR",        "Name",                    0xFFCCCCCC);
    draw_kv(&mut y, "0x000",     "—",                       0xFF888888);
    let mut sorted: alloc::vec::Vec<(u16, u64)> = s.syscalls.clone();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    for (nr, count) in &sorted {
        draw_kv(&mut y, &alloc::format!("0x{:03X}", nr),
                     &alloc::format!("{} ({})", syscall::name(*nr), count),
                     color_for_category(*nr));
    }
}

fn color_for_category(nr: u16) -> u32 {
    use crate::bmo_abi::syscalls;
    let n = nr as u32;
    if n >= syscalls::NR_WM_CREATE_WINDOW && n <= syscalls::NR_WM_PUMP_EVENTS { return 0xFF00FFAA; } // windowing
    if n >= syscalls::NR_DRAW_CLEAR && n <= syscalls::NR_WINPAINT_DRAW_CIRCLE { return 0xFFFFAA00; } // drawing
    if n >= syscalls::NR_FS_OPEN && n <= syscalls::NR_FS_MOUNT { return 0xFFAAFF00; } // fs
    if n >= syscalls::NR_TIME_NOW_NS && n <= syscalls::NR_TIME_SLEEP_MS { return 0xFF00FFFF; } // time
    if n >= syscalls::NR_INPUT_POLL_KEY && n <= syscalls::NR_INPUT_POLL_EVENT { return 0xFFFF00FF; } // input
    if n >= syscalls::NR_AUDIO_PLAY && n <= syscalls::NR_AUDIO_LOAD_WAVE { return 0xFFFFFF00; } // audio
    if n >= syscalls::NR_PROC_SPAWN && n <= syscalls::NR_THREAD_SELF { return 0xFF00AAFF; } // proc
    if n >= syscalls::NR_MEM_ALLOC && n <= syscalls::NR_MEM_UNMAP { return 0xFFAA00FF; } // mem
    if n >= syscalls::NR_BEFCORE_SEND && n <= syscalls::NR_BEFCORE_REGISTER { return 0xFFFF8800; } // befcore
    if n >= syscalls::NR_IPC_PORT_CREATE && n <= syscalls::NR_IPC_PORT_CLOSE { return 0xFF88FF00; } // ipc
    if n >= syscalls::NR_SURFACE_MAP && n <= syscalls::NR_SURFACE_PRESENT { return 0xFF0088FF; } // surface
    if n >= syscalls::NR_DEBUG_PRINT && n <= syscalls::NR_DEBUG_PANIC { return 0xFFFF0000; } // debug
    0xFFCCCCCC
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF1A1A2E);
    draw_text(8, 8, "SYSCALL", 0xFF00FFFF);
    draw_text(80, 8, "— BMO ABI 0x100..0x1FF", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF202028);
    draw_text(8, *y + 2, title, 0xFF00FFFF);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.syscalls] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
