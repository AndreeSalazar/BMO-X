use crate::fb::{self, FrameBuffer};
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, s: &SystemSnapshot) {
    H::header(fb, "GATEWAY", 0xFFFF0080);
    let mut y = 40u32;

    let total: u64 = s.telemetry.syscall_counts.iter().sum();
    y = H::section(fb, y, "Stats (acumuladas desde boot)", 0xFFFF0080);
    y = H::kv_u64(fb, y, "Total syscalls", total, 0xFFFFFFFF);

    y = H::section(fb, y, "Pipeline (por syscall)", 0xFFFF0080);
    fb::draw_text(fb, 16, y, "1. Validate range (0x100..0x1FF)", 0xFF00FFFF);
    y += 16;
    fb::draw_text(fb, 16, y, "2. ByteDefender: capabilities", 0xFFFF00FF);
    y += 16;
    fb::draw_text(fb, 16, y, "3. Cabina: trace_u64(name, nr)", 0xFF00FFAA);
    y += 16;
    fb::draw_text(fb, 16, y, "4. bmo_api::dispatch_syscall", 0xFFAAFF00);
    y += 16;
    fb::draw_text(fb, 16, y, "5. Return rax to Ring 3 (iretq)", 0xFFFFFFFF);
    y += 24;

    y = H::section(fb, y, "About", 0xFFFF0080);
    y = H::line(fb, y, "bmo_core::desktop3 is the only door", 0xFFCCCCCC);
    y += 14;
    y = H::line(fb, y, "between Ring 0 and BMO Core.", 0xFFCCCCCC);
    y += 14;
    H::line(fb, y, "All 86 BMO ABI syscalls pass through here.", 0xFFCCCCCC);
}
