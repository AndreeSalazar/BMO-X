#![allow(dead_code)]

//! Framebuffer drawing — direct pixel access to VBE linear framebuffer.
//!
//! 1920×1080×32bpp at 0xD0000000, pitch 7680 bytes.
//! All writes are volatile (MMIO). Ring 0, no_std.

/// Framebuffer handle — raw pixel access.
pub struct Framebuffer {
    addr: usize,
    pitch: usize, // bytes per scanline
    pub width: usize,
    pub height: usize,
}

impl Framebuffer {
    pub fn new(addr: u64, pitch: u64, width: u32, height: u32) -> Self {
        let pitch = pitch as usize;
        let mut width = width as usize;
        let mut height = height as usize;

        // If this handle points at the real GOP framebuffer, cap its logical
        // dimensions by the byte size reported by UEFI. Several higher-level
        // renderers only know width/height/stride; this central guard prevents
        // a bad GOP mode or stale dimensions from writing past VRAM and
        // corrupting kernel memory, which on real hardware often looks like a
        // sudden reboot/triple fault with no diagnostic screen.
        unsafe {
            if addr != 0 && addr == crate::info::FB_ADDR && pitch != 0 {
                let fb_size = if crate::info::BOOT_INFO.is_null() {
                    0
                } else {
                    (*crate::info::BOOT_INFO).fb_size as usize
                };
                if fb_size != 0 {
                    let pitch_px = pitch / 4;
                    let max_rows = fb_size / pitch;
                    width = width.min(pitch_px);
                    height = height.min(max_rows);
                }
            }
        }

        Self {
            addr: addr as usize,
            pitch,
            width,
            height,
        }
    }

    /// Write a single pixel. No bounds check for speed.
    #[inline(always)]
    pub fn put_pixel(&self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height { return; }
        let off = y * (self.pitch / 4) + x;
        unsafe {
            (self.addr as *mut u32).add(off).write_volatile(color);
        }
    }

    /// Fill entire screen with one color.
    pub fn clear(&self, color: u32) {
        let buf = self.addr as *mut u32;
        let pitch_px = self.pitch / 4;
        for y in 0..self.height {
            for x in 0..self.width {
                unsafe { buf.add(y * pitch_px + x).write_volatile(color); }
            }
        }
    }

    /// Filled rectangle.
    pub fn fill_rect(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for row in y..(y + h).min(self.height) {
            for col in x..(x + w).min(self.width) {
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
            for row in y..(y + h).min(self.height) {
                self.put_pixel(x + col, row, color);
            }
        }
    }

    /// Vertical gradient rectangle (top_color → bottom_color).
    pub fn gradient_v(&self, x: usize, y: usize, w: usize, h: usize, top: u32, bottom: u32) {
        // v1.6.12: dithered gradient. Integer-only lerp on RGB produces
        // visible bands on dark gradients (e.g. #050B12 -> #0E1B2E).
        // We add ±1 to each channel based on (x*y) hash to break the bands.
        for row in 0..h {
            let t = row as u32;
            let inv = (h - 1).max(1) as u32;
            let base = lerp_color(top, bottom, t, inv);
            let br = (base >> 16) & 0xFF;
            let bg_c = (base >> 8) & 0xFF;
            let bb = base & 0xFF;
            for col in x..(x + w).min(self.width) {
                // Tiny hash: pseudo-random ±1 per channel
                let h = ((col as u32).wrapping_mul(2654435761)
                       ^ (row as u32).wrapping_mul(40503)) & 0x7;
                let d = h as i32 - 3; // -3..+3
                let r = (br as i32 + d).clamp(0, 255) as u32;
                let g = (bg_c as i32 + d).clamp(0, 255) as u32;
                let b = (bb as i32 + d).clamp(0, 255) as u32;
                self.put_pixel(col, y + row, 0xFF000000 | (r << 16) | (g << 8) | b);
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
        for col in x..(x + w).min(self.width) {
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

    /// Fast blit (copy) of this framebuffer to a destination framebuffer.
    pub fn blit_to(&self, dest: &Self) {
        let src_ptr = self.addr as *const u32;
        let dst_ptr = dest.addr as *mut u32;
        if self.pitch == dest.pitch && self.width == dest.width && self.height == dest.height {
            let total_pixels = self.height * (self.pitch / 4);
            unsafe {
                core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, total_pixels);
            }
        } else {
            let copy_width = self.width.min(dest.width);
            let copy_height = self.height.min(dest.height);
            for y in 0..copy_height {
                unsafe {
                    let src_line = (self.addr + y * self.pitch) as *const u32;
                    let dst_line = (dest.addr + y * dest.pitch) as *mut u32;
                    core::ptr::copy_nonoverlapping(src_line, dst_line, copy_width);
                }
            }
        }
    }
    /// Read a pixel from the framebuffer (for alpha blending / blur).
    #[inline(always)]
    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x >= self.width || y >= self.height { return 0; }
        let off = y * (self.pitch / 4) + x;
        unsafe { (self.addr as *const u32).add(off).read_volatile() }
    }

    /// Alpha-blended pixel write: source-over-destination (A_over_B).
    #[inline(always)]
    pub fn put_pixel_alpha(&self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height { return; }
        let sa = (color >> 24) & 0xFF;
        if sa == 0 { return; }
        if sa == 255 { self.put_pixel(x, y, color); return; }
        let dst = self.get_pixel(x, y);
        let da = (dst >> 24) & 0xFF;
        let inv = 255 - sa;
        let r = (((color >> 16) & 0xFF) as u32 * sa + ((dst >> 16) & 0xFF) as u32 * inv) / 255;
        let g = (((color >> 8) & 0xFF) as u32 * sa + ((dst >> 8) & 0xFF) as u32 * inv) / 255;
        let b = ((color & 0xFF) as u32 * sa + (dst & 0xFF) as u32 * inv) / 255;
        let a = if sa + da > 255 { 255 } else { sa + da };
        self.put_pixel(x, y, (a << 24) | (r << 16) | (g << 8) | b);
    }

    /// Alpha-blended filled rectangle.
    pub fn fill_rect_alpha(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let sa = (color >> 24) & 0xFF;
        if sa == 0 { return; }
        if sa == 255 { self.fill_rect(x, y, w, h, color); return; }
        for row in y..(y + h).min(self.height) {
            for col in x..(x + w).min(self.width) {
                self.put_pixel_alpha(col, row, color);
            }
        }
    }

    /// Draw rounded rectangle outline (border only, no fill).
    pub fn draw_rounded_rect(&self, x: usize, y: usize, w: usize, h: usize, r: usize, color: u32, thickness: usize) {
        if w < 2 * thickness || h < 2 * thickness { return; }
        // Top straight edge
        self.fill_rect(x + r, y, w.saturating_sub(2 * r), thickness, color);
        // Bottom straight edge
        self.fill_rect(x + r, y + h - thickness, w.saturating_sub(2 * r), thickness, color);
        // Left straight edge
        self.fill_rect(x, y + r, thickness, h.saturating_sub(2 * r), color);
        // Right straight edge
        self.fill_rect(x + w - thickness, y + r, thickness, h.saturating_sub(2 * r), color);
        // Corner arcs (approximate with circle outline)
        for _i in 0..thickness {
            self.draw_corner_arc(x + r, y + r, r, color, thickness, true, true);
            self.draw_corner_arc(x + w - r - 1, y + r, r, color, thickness, false, true);
            self.draw_corner_arc(x + r, y + h - r - 1, r, color, thickness, true, false);
            self.draw_corner_arc(x + w - r - 1, y + h - r - 1, r, color, thickness, false, false);
        }
    }

    fn draw_corner_arc(&self, cx: usize, cy: usize, r: usize, color: u32, t: usize, left: bool, top: bool) {
        let r2 = (r * r) as isize;
        let ri = r as isize;
        let inner_r2 = ((r.saturating_sub(t)) * (r.saturating_sub(t))) as isize;
        for dy in 0..=ri {
            for dx in 0..=ri {
                let dd = dx * dx + dy * dy;
                if dd <= r2 && dd >= inner_r2 {
                    let px = if left { cx as isize - dx } else { cx as isize + dx };
                    let py = if top { cy as isize - dy } else { cy as isize + dy };
                    if px >= 0 && py >= 0 {
                        self.put_pixel(px as usize, py as usize, color);
                    }
                }
            }
        }
    }

    /// 3x3 box blur over a region. Reads from the current framebuffer and
    /// writes the blurred result back. Lightweight, good enough for shadows.
    pub fn box_blur_3x3(&self, x: usize, y: usize, w: usize, h: usize) {
        let pitch_px = self.pitch / 4;
        let buf = self.addr as *mut u32;
        // Read all pixels into a temporary buffer first
        let mut tmp: [u32; 512] = [0; 512]; // Max blur width 512
        for row in y..(y + h).min(self.height) {
            for col in x..(x + w).min(self.width).min(512) {
                let idx = col - x;
                if idx >= 512 { break; }
                let off = row * pitch_px + col;
                tmp[idx] = unsafe { buf.add(off).read_volatile() };
            }
            // Write blurred back
            for col in x..(x + w).min(self.width).min(512) {
                let idx = col - x;
                if idx >= 512 { break; }
                let mut sum_r: u32 = 0; let mut sum_g: u32 = 0; let mut sum_b: u32 = 0; let mut n: u32 = 0;
                for _ddy in -1i32..=1 {
                    for ddx in -1i32..=1 {
                        let si = idx as i32 + ddx;
                        if si >= 0 && si < (w as i32).min(512) {
                            let p = tmp[si as usize];
                            sum_r += (p >> 16) & 0xFF;
                            sum_g += (p >> 8) & 0xFF;
                            sum_b += p & 0xFF;
                            n += 1;
                        }
                    }
                }
                let r = sum_r / n; let g = sum_g / n; let b = sum_b / n;
                let off = row * pitch_px + col;
                unsafe { buf.add(off).write_volatile(0xFF000000 | (r << 16) | (g << 8) | b); }
            }
        }
    }

    /// Alpha-blended blit of source pixels to this framebuffer.
    /// src_data: raw ARGB pixels, src_w × src_h.
    pub fn blit_alpha(&self, dst_x: usize, dst_y: usize, src_data: &[u32], src_w: usize, src_h: usize) {
        for sy in 0..src_h {
            let dy = dst_y + sy;
            if dy >= self.height { break; }
            for sx in 0..src_w {
                let dx = dst_x + sx;
                if dx >= self.width { break; }
                let color = src_data[sy * src_w + sx];
                self.put_pixel_alpha(dx, dy, color);
            }
        }
    }
}

/// Create a Framebuffer wrapping the static backbuffer memory.
pub fn backbuffer_fb() -> Framebuffer {
    let (width, height) = unsafe {
        let w = crate::info::FB_WIDTH;
        let h = crate::info::FB_HEIGHT;
        if w != 0 && h != 0 { (w as usize, h as usize) } else { (1920, 1080) }
    };
    let addr = crate::dev::framebuffer::backbuffer_ptr() as u64;
    Framebuffer {
        addr: addr as usize,
        pitch: width * 4,
        width,
        height,
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

    // Success/active green accent
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
