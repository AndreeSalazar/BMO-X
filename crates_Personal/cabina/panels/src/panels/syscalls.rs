use crate::fb::{self, FrameBuffer};
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

fn syscall_name(nr: u16) -> &'static str {
    match nr {
        0x00 => "null", 0x01 => "debug_print", 0x02 => "debug_panic",
        0x10 => "wm_create_window", 0x11 => "wm_destroy_window", 0x12 => "wm_resize",
        0x13 => "wm_move", 0x14 => "wm_set_title", 0x15 => "wm_pump_events",
        0x20 => "draw_clear", 0x21 => "draw_rect", 0x22 => "draw_text",
        0x30 => "fs_open", 0x31 => "fs_read", 0x32 => "fs_write",
        0x33 => "fs_close", 0x34 => "fs_stat", 0x35 => "fs_mount",
        0x40 => "time_now_ns", 0x41 => "time_sleep_ms",
        0x50 => "input_poll_key", 0x51 => "input_poll_event",
        0x60 => "audio_play", 0x61 => "audio_load_wave",
        0x70 => "proc_spawn", 0x71 => "proc_exit", 0x72 => "thread_self",
        0x80 => "mem_alloc", 0x81 => "mem_free", 0x82 => "mem_map", 0x83 => "mem_unmap",
        0x90 => "befcore_send", 0x91 => "befcore_register",
        0xA0 => "ipc_port_create", 0xA1 => "ipc_port_send",
        0xA2 => "ipc_port_recv", 0xA3 => "ipc_port_close",
        0xB0 => "surface_map", 0xB1 => "surface_present",
        _ => "unknown",
    }
}

fn color_for_category(nr: u16) -> u32 {
    match nr >> 4 {
        0x0 => 0xFFFF0000,
        0x1 => 0xFF00FFAA,
        0x2 => 0xFFFFAA00,
        0x3 => 0xFFAAFF00,
        0x4 => 0xFF00FFFF,
        0x5 => 0xFFFF00FF,
        0x6 => 0xFFFFFF00,
        0x7 => 0xFF00AAFF,
        0x8 => 0xFFAA00FF,
        0x9 => 0xFFFF8800,
        0xA => 0xFF88FF00,
        0xB => 0xFF0088FF,
        _ => 0xFFCCCCCC,
    }
}

pub fn render(fb: &mut dyn FrameBuffer, s: &SystemSnapshot) {
    H::header(fb, "SYSCALL", 0xFF00FFFF);
    let mut y = 40u32;

    let syscall_counts = &s.telemetry.syscall_counts;
    let mut pairs: alloc::vec::Vec<(u16, u64)> = alloc::vec::Vec::new();
    for (nr, &count) in syscall_counts.iter().enumerate() {
        if count > 0 {
            pairs.push((nr as u16, count));
        }
    }
    let total: u64 = pairs.iter().map(|(_, c)| c).sum();
    y = H::section(fb, y, "Summary", 0xFF00FFFF);
    y = H::kv_u64(fb, y, "Total syscalls", total, 0xFF00FF00);
    y = H::kv_u64(fb, y, "Unique called", pairs.len() as u64, 0xFFFFFFFF);

    y = H::section(fb, y, "Per-syscall (sorted)", 0xFF00FFFF);
    fb::draw_text(fb, 16, y, "NR", 0xFFCCCCCC);
    fb::draw_text(fb, 80, y, "Name", 0xFFCCCCCC);
    fb::draw_text(fb, 360, y, "Count", 0xFFCCCCCC);
    y += 16;
    pairs.sort_by(|a, b| b.1.cmp(&a.1));
    for (nr, count) in &pairs {
        let nr_str = alloc::format!("0x{:03X}", nr);
        let name = syscall_name(*nr);
        fb::draw_text(fb, 16, y, &nr_str, 0xFFCCCCCC);
        fb::draw_text(fb, 80, y, &alloc::format!("{} ({})", name, count), color_for_category(*nr));
        fb::draw_text(fb, 360, y, &alloc::format!("{}", count), 0xFFFFFFFF);
        y += 14;
        if y > 1000 {
            break;
        }
    }
}
