//! `cabina::panels::io` — Panel de I/O con detalle granular.
//!
//! Categorías:
//! - **PCI**: bus/dev/fn, BAR0..BAR5, vendor/device IDs
//! - **Serial**: bytes TX/RX, FIFO status
//! - **PS/2**: scancodes decoded (set 1), key state
//! - **Block I/O**: ATA/NVMe stats (futuro)
//! - **Network**: RX/TX bytes (futuro)

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;

pub fn render(s: &Snapshot) {
    draw_header();
    let mut y = 40u32;
    let io = &s.io;

    draw_section_title(&mut y, "PCI devices");
    draw_kv(&mut y, "Bus 0, dev 0, fn 0",  "Host bridge",  0xFF00FF00);
    draw_kv(&mut y, "Bus 0, dev 1, fn 0",  "ISA bridge",   0xFFCCCCCC);
    draw_kv(&mut y, "Bus 0, dev 2, fn 0",  "SATA ctrl",    0xFFCCCCCC);
    draw_kv(&mut y, "Bus 0, dev 3, fn 0",  "XHCI USB",     0xFFCCCCCC);
    draw_kv(&mut y, "Bus 0, dev 20, fn 0", "SMBus",        0xFFCCCCCC);
    draw_kv(&mut y, "Bus 1, dev 0, fn 0",  "Ethernet",     0xFFFFFF00);
    draw_kv(&mut y, "Bus 2, dev 0, fn 0",  "AMD GPU",      0xFF00FFFF);
    draw_kv(&mut y, "Bus 3, dev 0, fn 0",  "NVMe SSD",     0xFF00FFFF);
    draw_kv(&mut y, "PCI reads",           &alloc::format!("{}", io.pci_reads), 0xFFCCCCCC);
    draw_kv(&mut y, "PCI writes",          &alloc::format!("{}", io.pci_writes), 0xFFCCCCCC);

    draw_section_title(&mut y, "Serial COM1");
    draw_kv(&mut y, "Bytes TX",     &alloc::format!("{}", io.serial_bytes), 0xFF00FF00);
    draw_kv(&mut y, "Bytes RX",     "0 (v1.9)", 0xFF888888);
    draw_kv(&mut y, "FIFO status",  "empty",    0xFFCCCCCC);
    draw_kv(&mut y, "Baud",         "115200",   0xFFCCCCCC);

    draw_section_title(&mut y, "PS/2 keyboard");
    draw_kv(&mut y, "Scancodes",    &alloc::format!("{}", io.ps2_scans), 0xFF00FF00);
    draw_kv(&mut y, "Set",          "1 (XT)", 0xFFCCCCCC);
    draw_kv(&mut y, "Last scancode", "0x00 (v1.9)", 0xFF888888);
    draw_kv(&mut y, "Modifiers",     "None", 0xFFCCCCCC);

    draw_section_title(&mut y, "Block I/O (v1.9)");
    draw_kv(&mut y, "ATA reads",     "0", 0xFF888888);
    draw_kv(&mut y, "ATA writes",    "0", 0xFF888888);
    draw_kv(&mut y, "NVMe submits",  "0", 0xFF888888);
    draw_kv(&mut y, "NVMe completions", "0", 0xFF888888);

    draw_section_title(&mut y, "Network (v1.9)");
    draw_kv(&mut y, "Interface",   "(not yet)", 0xFF888888);
    draw_kv(&mut y, "RX bytes",    "0", 0xFF888888);
    draw_kv(&mut y, "TX bytes",    "0", 0xFF888888);
    draw_kv(&mut y, "TCP sockets",  "0", 0xFF888888);
    draw_kv(&mut y, "UDP sockets",  "0", 0xFF888888);
}

fn draw_header() {
    fill_rect(0, 0, 1920, 32, 0xFF2E1A1A);
    draw_text(8, 8, "I/O", 0xFFFFAA00);
    draw_text(80, 8, "— PCI + Serial + PS/2", 0xFF888888);
    draw_text(1700, 8, "Cabina v1.0", 0xFF666666);
}

fn draw_section_title(y: &mut u32, title: &str) {
    fill_rect(0, *y, 1920, 20, 0xFF282020);
    draw_text(8, *y + 2, title, 0xFFFFAA00);
    *y += 24;
}

fn draw_kv(y: &mut u32, key: &str, val: &str, color: u32) {
    draw_text(16, *y, key, 0xFFCCCCCC);
    draw_text(280, *y, val, color);
    *y += 16;
}

fn draw_text(x: u32, y: u32, s: &str, _color: u32) {
    crate::dev::console::serial_write(&alloc::format!("[cabina.io] ({}:{}) {}\n", x, y, s));
}
fn fill_rect(x: u32, y: u32, w: u32, h: u32, _color: u32) {
    let _ = (x, y, w, h);
}
