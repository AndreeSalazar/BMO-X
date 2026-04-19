//! Framebuffer drawing — direct pixel access to VBE linear framebuffer.
//!
//! 1920×1080×32bpp at 0xD0000000, pitch 7680 bytes.
//! All writes are volatile (MMIO). Ring 0, no_std.

const FB_WIDTH: usize = 1920;
const FB_HEIGHT: usize = 1080;

/// Framebuffer handle — raw pixel access.
pub struct Framebuffer {
    addr: usize,
    pitch: usize, // bytes per scanline
}

impl Framebuffer {
    pub fn new(addr: u64, pitch: u64) -> Self {
        Self {
            addr: addr as usize,
            pitch: pitch as usize,
        }
    }

    /// Write a single pixel. No bounds check for speed.
    #[inline(always)]
    pub fn put_pixel(&self, x: usize, y: usize, color: u32) {
        if x >= FB_WIDTH || y >= FB_HEIGHT { return; }
        let off = y * (self.pitch / 4) + x;
        unsafe {
            (self.addr as *mut u32).add(off).write_volatile(color);
        }
    }

    /// Fill entire screen with one color.
    pub fn clear(&self, color: u32) {
        let buf = self.addr as *mut u32;
        let pitch_px = self.pitch / 4;
        for y in 0..FB_HEIGHT {
            for x in 0..FB_WIDTH {
                unsafe { buf.add(y * pitch_px + x).write_volatile(color); }
            }
        }
    }

    /// Filled rectangle.
    pub fn fill_rect(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for row in y..(y + h).min(FB_HEIGHT) {
            for col in x..(x + w).min(FB_WIDTH) {
                self.put_pixel(col, row, color);
            }
        }
    }

    /// Rectangle outline (border only).
    pub fn draw_rect(&self, x: usize, y: usize, w: usize, h: usize, color: u32, thickness: usize) {
        // Top
        self.fill_rect(x, y, w, thickness, color);
        // Bottom
        self.fill_rect(x, y + h - thickness, w, thickness, color);
        // Left
        self.fill_rect(x, y, thickness, h, color);
        // Right
        self.fill_rect(x + w - thickness, y, thickness, h, color);
    }

    /// Horizontal gradient rectangle (left_color → right_color).
    pub fn gradient_h(&self, x: usize, y: usize, w: usize, h: usize, left: u32, right: u32) {
        for col in 0..w {
            let t = col as u32;
            let inv = (w - 1) as u32;
            let color = lerp_color(left, right, t, inv);
            for row in y..(y + h).min(FB_HEIGHT) {
                self.put_pixel(x + col, row, color);
            }
        }
    }

    /// Vertical gradient rectangle (top_color → bottom_color).
    pub fn gradient_v(&self, x: usize, y: usize, w: usize, h: usize, top: u32, bottom: u32) {
        for row in 0..h {
            let t = row as u32;
            let inv = (h - 1).max(1) as u32;
            let color = lerp_color(top, bottom, t, inv);
            for col in x..(x + w).min(FB_WIDTH) {
                self.put_pixel(col, y + row, color);
            }
        }
    }

    /// Filled circle (midpoint algorithm).
    pub fn fill_circle(&self, cx: usize, cy: usize, r: usize, color: u32) {
        let r2 = (r * r) as isize;
        let ri = r as isize;
        for dy in -(ri)..=ri {
            for dx in -(ri)..=ri {
                if dx * dx + dy * dy <= r2 {
                    let px = cx as isize + dx;
                    let py = cy as isize + dy;
                    if px >= 0 && py >= 0 {
                        self.put_pixel(px as usize, py as usize, color);
                    }
                }
            }
        }
    }

    /// Horizontal line.
    pub fn hline(&self, x: usize, y: usize, w: usize, color: u32) {
        for col in x..(x + w).min(FB_WIDTH) {
            self.put_pixel(col, y, color);
        }
    }

    /// Rounded rectangle (filled with rounded corners).
    pub fn fill_rounded_rect(&self, x: usize, y: usize, w: usize, h: usize, r: usize, color: u32) {
        // Center body
        self.fill_rect(x + r, y, w - 2 * r, h, color);
        // Left strip
        self.fill_rect(x, y + r, r, h - 2 * r, color);
        // Right strip
        self.fill_rect(x + w - r, y + r, r, h - 2 * r, color);
        // Corners
        self.fill_corner(x + r, y + r, r, color, true, true);       // top-left
        self.fill_corner(x + w - r - 1, y + r, r, color, false, true);  // top-right
        self.fill_corner(x + r, y + h - r - 1, r, color, true, false);  // bottom-left
        self.fill_corner(x + w - r - 1, y + h - r - 1, r, color, false, false); // bottom-right
    }

    fn fill_corner(&self, cx: usize, cy: usize, r: usize, color: u32, left: bool, top: bool) {
        let r2 = (r * r) as isize;
        let ri = r as isize;
        for dy in 0..=ri {
            for dx in 0..=ri {
                if dx * dx + dy * dy <= r2 {
                    let px = if left { cx as isize - dx } else { cx as isize + dx };
                    let py = if top { cy as isize - dy } else { cy as isize + dy };
                    if px >= 0 && py >= 0 {
                        self.put_pixel(px as usize, py as usize, color);
                    }
                }
            }
        }
    }
}

/// Linear interpolation between two ARGB colors. t/total = blend factor.
fn lerp_color(a: u32, b: u32, t: u32, total: u32) -> u32 {
    if total == 0 { return a; }
    let ar = (a >> 16) & 0xFF;
    let ag = (a >> 8) & 0xFF;
    let ab = a & 0xFF;
    let br = (b >> 16) & 0xFF;
    let bg = (b >> 8) & 0xFF;
    let bb = b & 0xFF;

    let r = ar + (br.wrapping_sub(ar)).wrapping_mul(t) / total;
    let g = ag + (bg.wrapping_sub(ag)).wrapping_mul(t) / total;
    let bl = ab + (bb.wrapping_sub(ab)).wrapping_mul(t) / total;

    0xFF000000 | (r << 16) | (g << 8) | bl
}

// ── Color palette ──────────────────────────────────────────────────────────

pub mod colors {
    // Dark theme
    pub const BG_DARK: u32       = 0xFF0D1117;  // GitHub dark
    pub const BG_PANEL: u32      = 0xFF161B22;  // Panel bg
    pub const BG_CARD: u32       = 0xFF21262D;  // Card bg
    pub const BORDER: u32        = 0xFF30363D;  // Subtle border

    // NVIDIA green branding
    pub const NV_GREEN: u32      = 0xFF76B900;
    pub const NV_GREEN_DARK: u32 = 0xFF5A8F00;

    // Accent colors
    pub const ACCENT_BLUE: u32   = 0xFF58A6FF;
    pub const ACCENT_PURPLE: u32 = 0xFFBC8CFF;
    pub const ACCENT_CYAN: u32   = 0xFF56D4DD;
    pub const ACCENT_ORANGE: u32 = 0xFFD29922;
    pub const ACCENT_RED: u32    = 0xFFFF7B72;
    pub const ACCENT_PINK: u32   = 0xFFF778BA;

    // Text
    pub const TEXT_PRIMARY: u32  = 0xFFE6EDF3;
    pub const TEXT_SECONDARY: u32 = 0xFF8B949E;
    pub const TEXT_SUCCESS: u32  = 0xFF3FB950;

    // Status bar
    pub const BAR_BG: u32        = 0xFF1C2128;
    pub const BAR_FILL: u32      = 0xFF238636;

    pub const WHITE: u32         = 0xFFFFFFFF;
    pub const BLACK: u32         = 0xFF000000;
}
