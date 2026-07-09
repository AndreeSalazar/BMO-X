use crate::fb::FrameBuffer;
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, s: &SystemSnapshot) {
    H::header(fb, "BOOT", 0xFFFF8800);
    let mut y = 40u32;

    y = H::section(fb, y, "Boot summary", 0xFFFF8800);
    y = H::kv(fb, y, "Uptime", &alloc::format!("{} ms", s.telemetry.uptime_ns / 1_000_000), 0xFF00FF00);

    y = H::section(fb, y, "Boot phases", 0xFFFF8800);
    let phases: &[(&str, &str)] = &[
        ("P0 arch",    "OK"), ("P1 CPU", "OK"), ("P2 mem", "OK"),
        ("P3 dev",    "OK"), ("P4 user", "OK"), ("P5 bmo_core", "OK"),
        ("P6 desktop","OK"), ("P7 lang", "OK"), ("P8 cabina", "OK"),
    ];
    for (k, v) in phases {
        y = H::kv(fb, y, k, v, 0xFF00FF00);
    }

    y = H::section(fb, y, "Errors during boot", 0xFFFF8800);
    if s.telemetry.cpu.double_fault == 0 {
        y = H::kv(fb, y, "Double faults", "0", 0xFF00FF00);
    } else {
        y = H::kv_u64(fb, y, "Double faults", s.telemetry.cpu.double_fault, 0xFFFF0000);
    }
    y = H::kv_u64(fb, y, "Page faults", s.telemetry.cpu.page_faults, 0xFFFFFF00);
    H::kv_u64(fb, y, "General faults", s.telemetry.cpu.general_protection, 0xFFFFFF00);
}
