//! `cabina::panels::syscalls` — Panel de syscalls con detalle completo.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::telemetry::syscall;
use crate::cabina::panels::helpers as H;
use crate::cabina::paint;

pub fn render(s: &Snapshot) {
    H::header("SYSCALL", 0xFF00FFFF);

    let mut y = 40u32;

    y = H::section(y, "Summary", 0xFF00FFFF);
    y = H::kv_u64(y, "Total syscalls", syscall::get_total(), 0xFF00FF00);
    y = H::kv_u64(y, "Unique called",  s.syscalls.len() as u64, 0xFFFFFFFF);

    y = H::section(y, "Per-syscall (sorted by count)", 0xFF00FFFF);
    paint::draw_text(16, y, "NR", 0xFFCCCCCC);
    paint::draw_text(80, y, "Name", 0xFFCCCCCC);
    paint::draw_text(360, y, "Count", 0xFFCCCCCC);
    y += 16;
    let mut sorted: alloc::vec::Vec<(u16, u64)> = s.syscalls.clone();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    for (nr, count) in &sorted {
        let nr_str = alloc::format!("0x{:03X}", nr);
        let name = syscall::name(*nr);
        let line = alloc::format!("{} ({})", name, count);
        paint::draw_text(16, y, &nr_str, 0xFFCCCCCC);
        paint::draw_text(80, y, &line, color_for_category(*nr));
        paint::draw_text(360, y, &alloc::format!("{}", count), 0xFFFFFFFF);
        y += 14;
        if y > 1000 { break; }
    }
    let _ = y;
}

fn color_for_category(nr: u16) -> u32 {
    use crate::bmo_abi::syscalls;
    let n = nr as u32;
    if n >= syscalls::NR_WM_CREATE_WINDOW && n <= syscalls::NR_WM_PUMP_EVENTS { return 0xFF00FFAA; }
    if n >= syscalls::NR_DRAW_CLEAR && n <= syscalls::NR_WINPAINT_DRAW_CIRCLE { return 0xFFFFAA00; }
    if n >= syscalls::NR_FS_OPEN && n <= syscalls::NR_FS_MOUNT { return 0xFFAAFF00; }
    if n >= syscalls::NR_TIME_NOW_NS && n <= syscalls::NR_TIME_SLEEP_MS { return 0xFF00FFFF; }
    if n >= syscalls::NR_INPUT_POLL_KEY && n <= syscalls::NR_INPUT_POLL_EVENT { return 0xFFFF00FF; }
    if n >= syscalls::NR_AUDIO_PLAY && n <= syscalls::NR_AUDIO_LOAD_WAVE { return 0xFFFFFF00; }
    if n >= syscalls::NR_PROC_SPAWN && n <= syscalls::NR_THREAD_SELF { return 0xFF00AAFF; }
    if n >= syscalls::NR_MEM_ALLOC && n <= syscalls::NR_MEM_UNMAP { return 0xFFAA00FF; }
    if n >= syscalls::NR_BEFCORE_SEND && n <= syscalls::NR_BEFCORE_REGISTER { return 0xFFFF8800; }
    if n >= syscalls::NR_IPC_PORT_CREATE && n <= syscalls::NR_IPC_PORT_CLOSE { return 0xFF88FF00; }
    if n >= syscalls::NR_SURFACE_MAP && n <= syscalls::NR_SURFACE_PRESENT { return 0xFF0088FF; }
    if n >= syscalls::NR_DEBUG_PRINT && n <= syscalls::NR_DEBUG_PANIC { return 0xFFFF0000; }
    0xFFCCCCCC
}
