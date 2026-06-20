//! ÑEXO Runtime — E/S serial y framebuffer.
//!
//! Wraps kernel serial and framebuffer drivers.

#![allow(dead_code)]

/// Write a string to serial port (COM1).
pub fn serial_write(s: &str) {
    crate::dev::console::serial_write(s);
}

/// Write a single byte to serial.
pub fn serial_write_byte(b: u8) {
    crate::dev::console::serial_write_byte(b);
}

/// Try to read a byte from serial (non-blocking).
pub fn serial_read_byte() -> Option<u8> {
    crate::dev::console::serial_read_byte()
}

/// Framebuffer handle for graphics operations.
pub struct Graphics {
    fb: crate::bmo_core::ui::fb::Framebuffer,
}

impl Graphics {
    /// Create a graphics handle from the kernel framebuffer.
    pub fn new() -> Option<Self> {
        let (addr, stride, width, height) = unsafe {
            (crate::boot::info::FB_ADDR, crate::boot::info::FB_STRIDE, crate::boot::info::FB_WIDTH, crate::boot::info::FB_HEIGHT)
        };
        if addr == 0 {
            return None;
        }
        let fb = crate::bmo_core::ui::fb::Framebuffer::new(addr, stride as u64, width, height);
        Some(Self { fb })
    }

    /// Screen width in pixels.
    pub fn width(&self) -> usize {
        self.fb.width
    }

    /// Screen height in pixels.
    pub fn height(&self) -> usize {
        self.fb.height
    }

    /// Set a single pixel.
    pub fn pixel(&self, x: usize, y: usize, color: u32) {
        self.fb.put_pixel(x, y, color);
    }

    /// Fill entire screen with color.
    pub fn clear(&self, color: u32) {
        self.fb.clear(color);
    }

    /// Draw a filled rectangle.
    pub fn fill_rect(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        self.fb.fill_rect(x, y, w, h, color);
    }

    /// Draw a rectangle outline.
    pub fn draw_rect(&self, x: usize, y: usize, w: usize, h: usize, color: u32, thickness: usize) {
        self.fb.draw_rect(x, y, w, h, color, thickness);
    }

    /// Draw a horizontal line.
    pub fn hline(&self, x: usize, y: usize, w: usize, color: u32) {
        self.fb.hline(x, y, w, color);
    }

    /// Draw a filled circle.
    pub fn fill_circle(&self, cx: usize, cy: usize, r: usize, color: u32) {
        self.fb.fill_circle(cx, cy, r, color);
    }

    /// Draw a filled rounded rectangle.
    pub fn fill_rounded_rect(&self, x: usize, y: usize, w: usize, h: usize, r: usize, color: u32) {
        self.fb.fill_rounded_rect(x, y, w, h, r, color);
    }

    /// Draw text using VGA bitmap font (8x16 glyphs).
    pub fn draw_text(&self, x: usize, y: usize, text: &str, color: u32, scale: usize) {
        let mut cx = x;
        for byte in text.bytes() {
            let glyph = crate::bmo_core::ui::font::get_glyph(byte);
            for gy in 0..16 {
                let row = glyph[gy];
                for gx in 0..8 {
                    if row & (0x80 >> gx) != 0 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                self.fb.put_pixel(
                                    cx + gx * scale + sx,
                                    y + gy * scale + sy,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
            cx += 8 * scale;
        }
    }
}

/// PC speaker beep.
pub fn beep(freq: u32, duration_ms: u32) {
    crate::bmo_core::desktop::beep(freq, duration_ms);
}

/// Initialize I/O subsystem.
pub fn init() {
    crate::bmo_core::diag::info("nexo_io", "I/O subsystem initialized");
}
