use crate::fb::FrameBuffer;
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, s: &SystemSnapshot) {
    H::header(fb, "SCHED", 0xFFFFFF00);
    let mut y = 40u32;
    let sc = &s.telemetry.scheduler;

    y = H::section(fb, y, "Global", 0xFFFFFF00);
    y = H::kv_u64(fb, y, "Context switches", sc.context_switches, 0xFF00FF00);
    y = H::kv_u64(fb, y, "Processes",        sc.processes, 0xFFFFFFFF);
    H::kv_u64(fb, y, "Threads",          sc.threads, 0xFFCCCCCC);
}
