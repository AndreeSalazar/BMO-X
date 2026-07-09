use crate::fb::FrameBuffer;
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, s: &SystemSnapshot) {
    H::header(fb, "MEM", 0xFFAAFF00);
    let mut y = 40u32;
    let m = &s.telemetry.memory;

    y = H::section(fb, y, "Heap", 0xFFAAFF00);
    y = H::kv_size(fb, y, "Used",   m.heap_used, 0xFF00FF00);
    y = H::kv_size(fb, y, "Peak",   m.heap_peak, 0xFFFFFF00);
    y = H::kv_u64(fb, y, "Allocs",  m.allocations, 0xFFFFFFFF);
    y = H::kv_u64(fb, y, "Frees",   m.frees,  0xFFCCCCCC);
    y = H::kv_u64(fb, y, "Live",    m.allocations.saturating_sub(m.frees), 0xFF00FFFF);

    y = H::section(fb, y, "Pages", 0xFFAAFF00);
    y = H::kv_u64(fb, y, "Free",    m.free_pages, 0xFF00FF00);
    H::kv_size(fb, y, "Free KB", m.free_pages * 4096, 0xFFCCCCCC);
}
