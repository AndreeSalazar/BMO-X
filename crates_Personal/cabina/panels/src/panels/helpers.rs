use crate::fb::{self, FrameBuffer};

pub fn section(fb: &mut dyn FrameBuffer, y: u32, title: &str, color: u32) -> u32 {
    fb.fill_rect(0, y, fb.width(), 20, 0xFF202028);
    fb::draw_text(fb, 8, y + 2, title, color);
    y + 24
}

pub fn header(fb: &mut dyn FrameBuffer, title: &str, color: u32) {
    let w = fb.width();
    fb.fill_rect(0, 0, w, 32, 0xFF1A1A2E);
    fb::draw_text(fb, 8, 8, title, color);
    let subtitle = alloc::format!("-- Cabina v{}", env!("CARGO_PKG_VERSION"));
    fb::draw_text(fb, 80, 8, &subtitle, 0xFF888888);
    fb::draw_text_right(fb, w - 8, 8, "FastOS", 0xFF666666);
}

pub fn kv(fb: &mut dyn FrameBuffer, y: u32, key: &str, val: &str, color: u32) -> u32 {
    fb::draw_text(fb, 16, y, key, 0xFFCCCCCC);
    fb::draw_text(fb, 280, y, val, color);
    y + 16
}

pub fn kv_u64(fb: &mut dyn FrameBuffer, y: u32, key: &str, val: u64, color: u32) -> u32 {
    kv(fb, y, key, &alloc::format!("{}", val), color)
}

pub fn kv_size(fb: &mut dyn FrameBuffer, y: u32, key: &str, val: u64, color: u32) -> u32 {
    let s = if val < 1024 {
        alloc::format!("{} B", val)
    } else if val < 1024 * 1024 {
        alloc::format!("{} KB", val / 1024)
    } else {
        alloc::format!("{} MB", val / 1024 / 1024)
    };
    kv(fb, y, key, &s, color)
}

pub fn line(fb: &mut dyn FrameBuffer, y: u32, text: &str, color: u32) -> u32 {
    fb::draw_text(fb, 16, y, text, color);
    y + 16
}
