//! Cabina panels: uno por categoría, todos comparten `helpers::H`.
//!
//! `render(tab, &Snapshot)` es el dispatcher que se llama desde
//! `cabina::overlay`. `tab` ∈ 0..=10 (ver `PANEL_COUNT`).

use crate::cabina::snapshot::Snapshot;

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

pub const PANEL_COUNT: usize = 11;

pub const PANEL_NAMES: [&str; PANEL_COUNT] = [
    "OVERVIEW", "CPU", "MEM", "I/O", "SCHED",
    "SYSCALL", "EVENTS", "GPU", "BOOT", "LANG", "QUERY",
];

pub const PANEL_COLORS: [u32; PANEL_COUNT] = [
    0xFF00FFAA, 0xFF00FFAA, 0xFFAAFF00, 0xFFFFAA00, 0xFFFFFF00,
    0xFF00FFFF, 0xFFFF00FF, 0xFF00FFFF, 0xFFFF8800, 0xFFAAFF00, 0xFF44FF44,
];

/// Renderiza el panel `tab` sobre el framebuffer.
pub fn render(tab: u8, s: &Snapshot) {
    match tab {
        0  => overview::render(s),
        1  => cpu::render(s),
        2  => mem::render(s),
        3  => io::render(s),
        4  => sched::render(s),
        5  => syscalls::render(s),
        6  => events::render(s),
        7  => gpu::render(s),
        8  => boot::render(s),
        9  => lang::render(s),
        10 => query::render(s),
        _  => overview::render(s),
    }
}

/// Nombre del panel `tab`.
pub fn name(tab: u8) -> &'static str {
    match tab {
        0  => "OVERVIEW",
        1  => "CPU",
        2  => "MEM",
        3  => "I/O",
        4  => "SCHED",
        5  => "SYSCALL",
        6  => "EVENTS",
        7  => "GPU",
        8  => "BOOT",
        9  => "LANG",
        10 => "QUERY",
        _  => "OVERVIEW",
    }
}
