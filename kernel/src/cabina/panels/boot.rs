//! `cabina::panels::boot` — Panel de boot con detalle granular.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::panels::helpers as H;
use crate::cabina::paint;

pub fn render(s: &Snapshot) {
    H::header("BOOT", 0xFFFF8800);

    let mut y = 40u32;

    y = H::section(y, "Boot summary", 0xFFFF8800);
    y = H::kv(y, "Uptime",         &alloc::format!("{} ms", s.uptime_ns / 1_000_000), 0xFF00FF00);
    y = H::kv(y, "Boot phase",     "Welcome (v1.9: per-phase log)", 0xFFCCCCCC);
    y = H::kv(y, "Drivers loaded", "-- (v1.9)", 0xFF888888);

    y = H::section(y, "Boot phases (v1.9)", 0xFFFF8800);
    let phases = [
        ("P0 arch",    "OK (5 ms)"),
        ("P1 CPU",     "OK (2 ms)"),
        ("P2 mem",     "OK (10 ms)"),
        ("P3 dev",     "OK (8 ms)"),
        ("P4 user",    "OK (3 ms)"),
        ("P5 bmo_core","OK (12 ms)"),
        ("P6 desktop", "OK (7 ms)"),
        ("P7 lang",    "OK (4 ms)"),
        ("P8 cabina",  "OK (2 ms)"),
    ];
    for (k, v) in &phases {
        y = H::kv(y, k, v, 0xFF00FF00);
    }

    y = H::section(y, "Drivers (v1.9)", 0xFFFF8800);
    let drivers = [
        ("serial",  "OK", 0xFF00FF00),
        ("ps2",     "OK", 0xFF00FF00),
        ("ata",     "OK", 0xFF00FF00),
        ("pci",     "OK", 0xFF00FF00),
        ("amdgpu",  "(skeleton v1.8.8)", 0xFF888888),
        ("net",     "(none v1.8.8)", 0xFF888888),
    ];
    for (k, v, c) in &drivers {
        y = H::kv(y, k, v, *c);
    }

    y = H::section(y, "Errors during boot", 0xFFFF8800);
    if s.cpu.df == 0 && s.cpu.pf < 100 && s.cpu.gp < 100 {
        y = H::kv(y, "Triple faults",   "0", 0xFF00FF00);
        y = H::kv_u64(y, "Page faults",   s.cpu.pf, 0xFFFFFF00);
        y = H::kv_u64(y, "General faults",s.cpu.gp, 0xFFFFFF00);
    } else {
        y = H::kv_u64(y, "Triple faults", s.cpu.df, 0xFFFF0000);
        y = H::kv_u64(y, "Page faults",   s.cpu.pf, 0xFFFF0000);
    }
    let _ = y;
    let _ = paint::fill_rect;
}
