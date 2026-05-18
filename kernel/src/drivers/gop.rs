//! GOP Display Driver — 2D framebuffer rendering via UEFI GOP.
//!
//! Uses the framebuffer passed by the UEFI bootloader (already in fb.rs).
//! Provides basic 2D primitives: pixel, rect, blit, clear, text.

#![allow(dead_code)]

/// ARGB color (32-bit: 0xAARRGGBB).
#[derive(Debug, Clone, Copy)]
pub struct Color(pub u32);

impl Color {
    pub const BLACK:   Color = Color(0xFF000000);
    pub const WHITE:   Color = Color(0xFFFFFFFF);
    pub const RED:     Color = Color(0xFFFF0000);
    pub const GREEN:   Color = Color(0xFF00FF00);
    pub const BLUE:    Color = Color(0xFF0000FF);
    pub const YELLOW:  Color = Color(0xFFFFFF00);
    pub const CYAN:    Color = Color(0xFF00FFFF);
    pub const MAGENTA: Color = Color(0xFFFF00FF);
    pub const GRAY:    Color = Color(0xFF808080);
    pub const DARK_BG: Color = Color(0xFF1A1A2E);
    pub const ACCENT:  Color = Color(0xFF16213E);
    pub const HIGHLIGHT: Color = Color(0xFF0F3460);
    pub const NEON:    Color = Color(0xFFE94560);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color(0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }
}

/// GOP framebuffer display state.
pub struct GopDisplay {
    pub base: *mut u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32, // pixels per scanline (may be > width due to padding)
}

impl GopDisplay {
    /// Create display from boot info framebuffer parameters.
    pub fn from_boot_info(fb_base: u64, width: u32, height: u32, stride: u32) -> Self {
        Self {
            base: fb_base as *mut u32,
            width,
            height,
            stride,
        }
    }

    /// Draw a single pixel.
    #[inline]
    pub fn draw_pixel(&self, x: u32, y: u32, color: Color) {
        if x < self.width && y < self.height {
            unsafe {
                let offset = (y * self.stride + x) as isize;
                self.base.offset(offset).write_volatile(color.0);
            }
        }
    }

    /// Fill a rectangle.
    pub fn fill_rect(&self, x: u32, y: u32, w: u32, h: u32, color: Color) {
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);
        for row in y..y_end {
            for col in x..x_end {
                unsafe {
                    let offset = (row * self.stride + col) as isize;
                    self.base.offset(offset).write_volatile(color.0);
                }
            }
        }
    }

    /// Draw a horizontal line.
    pub fn hline(&self, x: u32, y: u32, w: u32, color: Color) {
        self.fill_rect(x, y, w, 1, color);
    }

    /// Draw a vertical line.
    pub fn vline(&self, x: u32, y: u32, h: u32, color: Color) {
        self.fill_rect(x, y, 1, h, color);
    }

    /// Draw a rectangle outline (1px border).
    pub fn draw_rect(&self, x: u32, y: u32, w: u32, h: u32, color: Color) {
        self.hline(x, y, w, color);              // top
        self.hline(x, y + h - 1, w, color);      // bottom
        self.vline(x, y, h, color);               // left
        self.vline(x + w - 1, y, h, color);       // right
    }

    /// Blit a buffer of ARGB pixels to the framebuffer.
    pub fn blit(&self, x: u32, y: u32, w: u32, h: u32, buf: &[u32]) {
        for row in 0..h {
            let dst_y = y + row;
            if dst_y >= self.height { break; }
            for col in 0..w {
                let dst_x = x + col;
                if dst_x >= self.width { break; }
                let src_idx = (row * w + col) as usize;
                if src_idx < buf.len() {
                    unsafe {
                        let offset = (dst_y * self.stride + dst_x) as isize;
                        self.base.offset(offset).write_volatile(buf[src_idx]);
                    }
                }
            }
        }
    }

    /// Clear the entire screen.
    pub fn clear(&self, color: Color) {
        for row in 0..self.height {
            for col in 0..self.width {
                unsafe {
                    let offset = (row * self.stride + col) as isize;
                    self.base.offset(offset).write_volatile(color.0);
                }
            }
        }
    }

    /// Draw a gradient background (dark theme).
    pub fn draw_gradient_bg(&self) {
        for row in 0..self.height {
            let t = row as f64 / self.height as f64;
            let r = (26.0 + t * 15.0) as u8;   // 0x1A → 0x29
            let g = (26.0 + t * 8.0) as u8;    // 0x1A → 0x22
            let b = (46.0 + t * 50.0) as u8;   // 0x2E → 0x60
            let color = Color::rgb(r, g, b);
            for col in 0..self.width {
                unsafe {
                    let offset = (row * self.stride + col) as isize;
                    self.base.offset(offset).write_volatile(color.0);
                }
            }
        }
    }
}

/// Global GOP display instance.
static mut GOP_DISPLAY: Option<GopDisplay> = None;

/// Initialize the global GOP display from boot info.
pub fn init_gop(fb_base: u64, width: u32, height: u32, stride: u32) {
    unsafe {
        GOP_DISPLAY = Some(GopDisplay::from_boot_info(fb_base, width, height, stride));
    }
}

/// Get a reference to the global GOP display.
pub fn display() -> Option<&'static GopDisplay> {
    unsafe { GOP_DISPLAY.as_ref() }
}
