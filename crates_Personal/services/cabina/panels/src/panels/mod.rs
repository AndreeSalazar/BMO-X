use crate::fb::FrameBuffer;
use cabina_core::SystemSnapshot;

pub mod helpers;
pub mod overview;
pub mod cpu;
pub mod mem;
pub mod io;
pub mod sched;
pub mod syscalls;
pub mod events;
pub mod boot;
pub mod lang;
pub mod query;
pub mod gpu;
pub mod gateway;

pub const PANEL_COUNT: usize = 12;

pub const PANEL_NAMES: [&str; PANEL_COUNT] = [
    "OVERVIEW", "CPU", "MEM", "I/O", "SCHED",
    "SYSCALL", "EVENTS", "GPU", "BOOT", "LANG", "QUERY", "GATEWAY",
];

pub const PANEL_COLORS: [u32; PANEL_COUNT] = [
    0xFF00FFAA, 0xFF00FFAA, 0xFFAAFF00, 0xFFFFAA00, 0xFFFFFF00,
    0xFF00FFFF, 0xFFFF00FF, 0xFF00FFFF, 0xFFFF8800, 0xFFAAFF00, 0xFF44FF44,
    0xFFFF0080,
];

pub fn render(fb: &mut dyn FrameBuffer, tab: u8, s: &SystemSnapshot) {
    match tab {
        0 => overview::render(fb, s),
        1 => cpu::render(fb, s),
        2 => mem::render(fb, s),
        3 => io::render(fb, s),
        4 => sched::render(fb, s),
        5 => syscalls::render(fb, s),
        6 => events::render(fb, s),
        7 => gpu::render(fb, s),
        8 => boot::render(fb, s),
        9 => lang::render(fb, s),
        10 => query::render(fb, s),
        11 => gateway::render(fb, s),
        _ => overview::render(fb, s),
    }
}

pub fn name(tab: u8) -> &'static str {
    match tab {
        0 => "OVERVIEW",
        1 => "CPU",
        2 => "MEM",
        3 => "I/O",
        4 => "SCHED",
        5 => "SYSCALL",
        6 => "EVENTS",
        7 => "GPU",
        8 => "BOOT",
        9 => "LANG",
        10 => "QUERY",
        11 => "GATEWAY",
        _ => "OVERVIEW",
    }
}
