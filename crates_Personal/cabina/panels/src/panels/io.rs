use crate::fb::FrameBuffer;
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, s: &SystemSnapshot) {
    H::header(fb, "I/O", 0xFFFFAA00);
    let mut y = 40u32;
    let io = &s.telemetry.io;

    y = H::section(fb, y, "PCI devices", 0xFFFFAA00);
    let devices: &[(&str, &str, u32)] = &[
        ("Bus 0, dev 0, fn 0",  "Host bridge",  0xFF00FF00),
        ("Bus 0, dev 1, fn 0",  "ISA bridge",   0xFFCCCCCC),
        ("Bus 0, dev 2, fn 0",  "SATA ctrl",    0xFFCCCCCC),
        ("Bus 0, dev 3, fn 0",  "XHCI USB",     0xFFCCCCCC),
        ("Bus 0, dev 20, fn 0", "SMBus",        0xFFCCCCCC),
        ("Bus 1, dev 0, fn 0",  "Ethernet",     0xFFFFFF00),
        ("Bus 2, dev 0, fn 0",  "AMD GPU",      0xFF00FFFF),
        ("Bus 3, dev 0, fn 0",  "NVMe SSD",     0xFF00FFFF),
    ];
    for (k, v, c) in devices {
        y = H::kv(fb, y, k, v, *c);
    }
    y = H::kv_u64(fb, y, "PCI reads",  io.pci_reads,  0xFFCCCCCC);
    y = H::kv_u64(fb, y, "PCI writes", io.pci_writes, 0xFFCCCCCC);

    y = H::section(fb, y, "Serial COM1", 0xFFFFAA00);
    y = H::kv_u64(fb, y, "Bytes TX", io.serial_bytes, 0xFF00FF00);
    H::kv(fb, y, "PS/2 scancodes", &alloc::format!("{}", io.ps2_scancodes), 0xFF00FF00);
}
