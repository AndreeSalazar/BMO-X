pub trait FrameBuffer {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn put_pixel(&mut self, x: u32, y: u32, color: u32);
    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32);
    fn glyph(&self, ch: u8) -> &[u8; 16];
    fn now_ns(&self) -> u64;
}

pub fn draw_text(fb: &mut dyn FrameBuffer, x: u32, y: u32, s: &str, color: u32) {
    let mut cx = x;
    for byte in s.bytes() {
        let g = *fb.glyph(byte);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..8u32 {
                if (bits >> (7 - col)) & 1 != 0 {
                    fb.put_pixel(cx + col, y + row as u32, color);
                }
            }
        }
        cx += 8;
        if cx >= fb.width() {
            break;
        }
    }
}

pub fn text_width(s: &str) -> u32 {
    s.len() as u32 * 8
}

pub const fn text_height() -> u32 {
    16
}

pub fn draw_text_right(fb: &mut dyn FrameBuffer, x_right: u32, y: u32, s: &str, color: u32) {
    let x = x_right.saturating_sub(text_width(s));
    draw_text(fb, x, y, s, color);
}
