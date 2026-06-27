//! `cabina::panels::io` — Panel de I/O con detalle granular.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::panels::helpers as H;
use crate::cabina::paint;

pub fn render(s: &Snapshot) {
    H::header("I/O", 0xFFFFAA00);

    let mut y = 40u32;
    let io = &s.io;

    y = H::section(y, "PCI devices", 0xFFFFAA00);
    let devices = [
        ("Bus 0, dev 0, fn 0",  "Host bridge",  0xFF00FF00),
        ("Bus 0, dev 1, fn 0",  "ISA bridge",   0xFFCCCCCC),
        ("Bus 0, dev 2, fn 0",  "SATA ctrl",    0xFFCCCCCC),
        ("Bus 0, dev 3, fn 0",  "XHCI USB",     0xFFCCCCCC),
        ("Bus 0, dev 20, fn 0", "SMBus",        0xFFCCCCCC),
        ("Bus 1, dev 0, fn 0",  "Ethernet",     0xFFFFFF00),
        ("Bus 2, dev 0, fn 0",  "AMD GPU",      0xFF00FFFF),
        ("Bus 3, dev 0, fn 0",  "NVMe SSD",     0xFF00FFFF),
    ];
    for (k, v, c) in &devices {
        y = H::kv(y, k, v, *c);
    }
    y = H::kv_u64(y, "PCI reads",  io.pci_reads,  0xFFCCCCCC);
    y = H::kv_u64(y, "PCI writes", io.pci_writes, 0xFFCCCCCC);

    y = H::section(y, "Serial COM1", 0xFFFFAA00);
    y = H::kv_u64(y, "Bytes TX",    io.serial_bytes, 0xFF00FF00);
    y = H::kv    (y, "Bytes RX",    "0 (v1.9)",      0xFF888888);
    y = H::kv    (y, "FIFO status", "empty",         0xFFCCCCCC);
    y = H::kv    (y, "Baud",        "115200",        0xFFCCCCCC);

    y = H::section(y, "PS/2 keyboard", 0xFFFFAA00);
    y = H::kv_u64(y, "Scancodes",     io.ps2_scans, 0xFF00FF00);
    y = H::kv    (y, "Set",           "1 (XT)",     0xFFCCCCCC);
    y = H::kv    (y, "Last scancode", "0x00 (v1.9)", 0xFF888888);
    y = H::kv    (y, "Modifiers",     "None",       0xFFCCCCCC);

    y = H::section(y, "Block I/O (v1.9)", 0xFFFFAA00);
    y = H::kv(y, "ATA reads",       "0", 0xFF888888);
    y = H::kv(y, "ATA writes",      "0", 0xFF888888);
    y = H::kv(y, "NVMe submits",    "0", 0xFF888888);
    y = H::kv(y, "NVMe completions","0", 0xFF888888);

    y = H::section(y, "Network (v1.9)", 0xFFFFAA00);
    y = H::kv(y, "Interface", "(not yet)", 0xFF888888);
    y = H::kv(y, "RX bytes",  "0", 0xFF888888);
    y = H::kv(y, "TX bytes",  "0", 0xFF888888);
    y = H::kv(y, "TCP sockets","0", 0xFF888888);
    y = H::kv(y, "UDP sockets","0", 0xFF888888);
    let _ = y;
    let _ = paint::fill_rect;
}
