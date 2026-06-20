#![allow(dead_code)]

//! GOP Display and Backbuffer Driver — UEFI GOP and static backbuffer management.
//!
//! Enhanced with:
//! - Double buffering (backbuffer → screen)
//! - Alpha blending
//! - Gradient fills
//! - Rounded rectangles
//! - Improved line drawing
//! - Anti-aliased rendering

use crate::bmo_core::ui::fb::Framebuffer;

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

    /// Extract alpha component (0-255)
    pub const fn alpha(self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }

    /// Extract red component
    pub const fn r(self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    /// Extract green component
    pub const fn g(self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    /// Extract blue component
    pub const fn b(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Blend this color with another (alpha blending)
    pub const fn blend(self, other: Color) -> Color {
        let a = self.alpha() as u32;
        let inv_a = 255 - a;
        let r = (self.r() as u32 * a + other.r() as u32 * inv_a) / 255;
        let g = (self.g() as u32 * a + other.g() as u32 * inv_a) / 255;
        let b = (self.b() as u32 * a + other.b() as u32 * inv_a) / 255;
        Color::rgb(r as u8, g as u8, b as u8)
    }

    /// Create gradient between two colors (t: 0.0-1.0)
    pub fn lerp(a: Color, b: Color, t: f32) -> Color {
        let t = t.max(0.0).min(1.0);
        let inv_t = 1.0 - t;
        let r = (a.r() as f32 * inv_t + b.r() as f32 * t) as u8;
        let g = (a.g() as f32 * inv_t + b.g() as f32 * t) as u8;
        let bl = (a.b() as f32 * inv_t + b.b() as f32 * t) as u8;
        Color::rgb(r, g, bl)
    }
}

/// GOP framebuffer display state.
#[derive(Debug, Clone, Copy)]
pub struct GopDisplay {
    pub base: *mut u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32, // pixels per scanline (may be > width due to padding)
}

static mut GOP_DISPLAY: Option<GopDisplay> = None;

/// Initialize the global GOP display from boot info.
pub fn init_gop(fb_base: u64, width: u32, height: u32, stride: u32) {
    unsafe {
        GOP_DISPLAY = Some(GopDisplay {
            base: fb_base as *mut u32,
            width,
            height,
            stride,
        });
    }
}

/// Get a reference to the global GOP display.
#[allow(static_mut_refs)]
pub fn display() -> Option<&'static GopDisplay> {
    unsafe { GOP_DISPLAY.as_ref() }
}

/// Put a pixel directly to framebuffer (no bounds check for speed)
#[inline]
pub fn put_pixel(x: u32, y: u32, color: Color) {
    if let Some(disp) = display() {
        if x < disp.width && y < disp.height {
            unsafe {
                let offset = (y * disp.stride + x) as usize;
                *disp.base.add(offset) = color.0;
            }
        }
    }
}

/// Get pixel from framebuffer
#[inline]
pub fn get_pixel(x: u32, y: u32) -> Color {
    if let Some(disp) = display() {
        if x < disp.width && y < disp.height {
            unsafe {
                let offset = (y * disp.stride + x) as usize;
                return Color(*disp.base.add(offset));
            }
        }
    }
    Color::BLACK
}

/// Fill a rectangle with solid color
pub fn fill_rect(x: u32, y: u32, w: u32, h: u32, color: Color) {
    if let Some(disp) = display() {
        for dy in 0..h {
            let py = y + dy;
            if py >= disp.height { break; }
            for dx in 0..w {
                let px = x + dx;
                if px >= disp.width { break; }
                unsafe {
                    let offset = (py * disp.stride + px) as usize;
                    *disp.base.add(offset) = color.0;
                }
            }
        }
    }
}

/// Fill a rectangle with horizontal gradient
pub fn fill_gradient_h(x: u32, y: u32, w: u32, h: u32, left: Color, right: Color) {
    if let Some(disp) = display() {
        for dx in 0..w {
            let t = dx as f32 / w as f32;
            let color = Color::lerp(left, right, t);
            for dy in 0..h {
                let py = y + dy;
                if py >= disp.height { continue; }
                let px = x + dx;
                if px >= disp.width { continue; }
                unsafe {
                    let offset = (py * disp.stride + px) as usize;
                    *disp.base.add(offset) = color.0;
                }
            }
        }
    }
}

/// Fill a rectangle with vertical gradient
pub fn fill_gradient_v(x: u32, y: u32, w: u32, h: u32, top: Color, bottom: Color) {
    if let Some(disp) = display() {
        for dy in 0..h {
            let t = dy as f32 / h as f32;
            let color = Color::lerp(top, bottom, t);
            let py = y + dy;
            if py >= disp.height { continue; }
            for dx in 0..w {
                let px = x + dx;
                if px >= disp.width { continue; }
                unsafe {
                    let offset = (py * disp.stride + px) as usize;
                    *disp.base.add(offset) = color.0;
                }
            }
        }
    }
}

/// Draw a rectangle outline (1px border)
pub fn draw_rect(x: u32, y: u32, w: u32, h: u32, color: Color) {
    // Top
    fill_rect(x, y, w, 1, color);
    // Bottom
    fill_rect(x, y + h - 1, w, 1, color);
    // Left
    fill_rect(x, y, 1, h, color);
    // Right
    fill_rect(x + w - 1, y, 1, h, color);
}

/// Draw a rounded rectangle
pub fn draw_rounded_rect(x: u32, y: u32, w: u32, h: u32, radius: u32, color: Color) {
    if let Some(disp) = display() {
        for py in y..y + h {
            if py >= disp.height { break; }
            for px in x..x + w {
                if px >= disp.width { break; }

                let dx = if px < x + radius {
                    radius - (px - x)
                } else if px >= x + w - radius {
                    (px - (x + w - radius)) + 1
                } else {
                    continue;
                };

                let dy = if py < y + radius {
                    radius - (py - y)
                } else if py >= y + h - radius {
                    (py - (y + h - radius)) + 1
                } else {
                    continue;
                };

                let dist_sq = dx * dx + dy * dy;
                if dist_sq <= radius * radius {
                    unsafe {
                        let offset = (py * disp.stride + px) as usize;
                        *disp.base.add(offset) = color.0;
                    }
                }
            }
        }
    }
}

/// Fill a rounded rectangle with solid color
pub fn fill_rounded_rect(x: u32, y: u32, w: u32, h: u32, radius: u32, color: Color) {
    if let Some(disp) = display() {
        for py in y..y + h {
            if py >= disp.height { break; }
            for px in x..x + w {
                if px >= disp.width { break; }

                let in_corner = (px < x + radius || px >= x + w - radius) &&
                                (py < y + radius || py >= y + h - radius);

                if in_corner {
                    let cx = if px < x + radius { x + radius } else { x + w - radius - 1 };
                    let cy = if py < y + radius { y + radius } else { y + h - radius - 1 };
                    let dx = (px as i32 - cx as i32).unsigned_abs();
                    let dy = (py as i32 - cy as i32).unsigned_abs();
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq > radius as u32 * radius as u32 {
                        continue;
                    }
                }

                unsafe {
                    let offset = (py * disp.stride + px) as usize;
                    *disp.base.add(offset) = color.0;
                }
            }
        }
    }
}

/// Draw a line using Bresenham's algorithm
pub fn draw_line(x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    loop {
        if x0 >= 0 && y0 >= 0 {
            put_pixel(x0 as u32, y0 as u32, color);
        }

        if x0 == x1 && y0 == y1 { break; }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x0 += sx;
        }
        if e2 < dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Draw a thick line (multiple pixels wide)
pub fn draw_line_thick(x0: i32, y0: i32, x1: i32, y1: i32, thickness: u32, color: Color) {
    // Draw perpendicular lines for thickness
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = (dx * dx + dy * dy) as f32;
    let len = sqrt_approx(len_sq);

    if len < 1.0 { return; }

    let nx = (-dy as f32 / len * thickness as f32 / 2.0) as i32;
    let ny = (dx as f32 / len * thickness as f32 / 2.0) as i32;

    // Draw filled polygon for thick line
    let points = [
        (x0 + nx, y0 + ny),
        (x1 + nx, y1 + ny),
        (x1 - nx, y1 - ny),
        (x0 - nx, y0 - ny),
    ];

    fill_polygon(&points, color);
}

/// Integer square root approximation
fn sqrt_approx(x: f32) -> f32 {
    if x <= 0.0 { return 0.0; }
    // Newton's method with initial guess
    let mut guess = x / 2.0;
    for _ in 0..16 {
        guess = (guess + x / guess) / 2.0;
    }
    guess
}

/// Fract function (fractional part)
fn fract(x: f32) -> f32 {
    x - floor(x)
}

/// Floor function
fn floor(x: f32) -> f32 {
    let i = x as i32;
    if x < 0.0 && x != i as f32 {
        (i - 1) as f32
    } else {
        i as f32
    }
}

/// Fill a convex polygon
pub fn fill_polygon(points: &[(i32, i32)], color: Color) {
    if points.len() < 3 { return; }

    // Find bounding box
    let mut min_x = points[0].0;
    let mut max_x = points[0].0;
    let mut min_y = points[0].1;
    let mut max_y = points[0].1;

    for &(x, y) in points.iter().skip(1) {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    // Scanline fill
    for y in min_y..=max_y {
        let mut intersections = [0i32; 32];
        let mut count = 0;

        for i in 0..points.len() {
            let j = (i + 1) % points.len();
            let (x0, y0) = points[i];
            let (x1, y1) = points[j];

            if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                let x = x0 + (y - y0) * (x1 - x0) / (y1 - y0);
                if count < 32 {
                    intersections[count] = x;
                    count += 1;
                }
            }
        }

        // Sort intersections
        for i in 0..count {
            for j in (i + 1)..count {
                if intersections[i] > intersections[j] {
                    intersections.swap(i, j);
                }
            }
        }

        // Fill between pairs
        let mut i = 0;
        while i + 1 < count {
            let x_start = intersections[i].max(0) as u32;
            let x_end = (intersections[i + 1] as u32).min(1919);
            fill_rect(x_start, y as u32, x_end - x_start + 1, 1, color);
            i += 2;
        }
    }
}

/// Draw a circle outline
pub fn draw_circle(cx: i32, cy: i32, radius: i32, color: Color) {
    let mut x = 0i32;
    let mut y = radius;
    let mut d = 3 - 2 * radius;

    while x <= y {
        put_pixel((cx + x) as u32, (cy + y) as u32, color);
        put_pixel((cx - x) as u32, (cy + y) as u32, color);
        put_pixel((cx + x) as u32, (cy - y) as u32, color);
        put_pixel((cx - x) as u32, (cy - y) as u32, color);
        put_pixel((cx + y) as u32, (cy + x) as u32, color);
        put_pixel((cx - y) as u32, (cy + x) as u32, color);
        put_pixel((cx + y) as u32, (cy - x) as u32, color);
        put_pixel((cx - y) as u32, (cy - x) as u32, color);

        if d < 0 {
            d += 4 * x + 6;
        } else {
            d += 4 * (x - y) + 10;
            y -= 1;
        }
        x += 1;
    }
}

/// Fill a circle
pub fn fill_circle(cx: i32, cy: i32, radius: i32, color: Color) {
    if let Some(disp) = display() {
        let r_sq = radius * radius;
        for y in -radius..=radius {
            for x in -radius..=radius {
                if x * x + y * y <= r_sq {
                    let px = cx + x;
                    let py = cy + y;
                    if px >= 0 && py >= 0 && (px as u32) < disp.width && (py as u32) < disp.height {
                        unsafe {
                            let offset = (py as u32 * disp.stride + px as u32) as usize;
                            *disp.base.add(offset) = color.0;
                        }
                    }
                }
            }
        }
    }
}

/// Draw an anti-aliased line (Wu's algorithm)
pub fn draw_line_aa(x0: f32, y0: f32, x1: f32, y1: f32, color: Color) {
    let steep = (y1 - y0).abs() > (x1 - x0).abs();

    let (mut x0, mut y0, mut x1, mut y1) = if steep {
        (y0, x0, y1, x1)
    } else {
        (x0, y0, x1, y1)
    };

    if x0 > x1 {
        core::mem::swap(&mut x0, &mut x1);
        core::mem::swap(&mut y0, &mut y1);
    }

    let dx = x1 - x0;
    let dy = y1 - y0;
    let gradient = if dx == 0.0 { 1.0 } else { dy / dx };

    let mut x = x0 as i32;
    let x_end = x1 as i32;
    let y = y0 as f32;

    let mut intery = y + gradient;

    while x <= x_end {
        let alpha = ((fract(intery) * 255.0) as u32).min(255);
        let alpha_inv = 255 - alpha;

        if steep {
            put_pixel(x as u32, intery as u32, Color::rgba(color.r(), color.g(), color.b(), alpha as u8));
            put_pixel(x as u32, intery as u32 + 1, Color::rgba(color.r(), color.g(), color.b(), alpha_inv as u8));
        } else {
            put_pixel(intery as u32, x as u32, Color::rgba(color.r(), color.g(), color.b(), alpha as u8));
            put_pixel(intery as u32 + 1, x as u32, Color::rgba(color.r(), color.g(), color.b(), alpha_inv as u8));
        }

        intery += gradient;
        x += 1;
    }
}

// ── Static Backbuffer ────────────────────────────────────────────────
// We define a static 8.29 MB backbuffer (1920x1080 resolution) in BSS.
// This prevents dynamic allocation failures due to physical memory fragmentation
// and guarantees a contiguous memory block at boot time.
const BACKBUFFER_SIZE: usize = 1920 * 1080;
static mut BACKBUFFER_MEM: [u32; BACKBUFFER_SIZE] = [0; BACKBUFFER_SIZE];

/// Get a Framebuffer representing the static backbuffer.
#[allow(static_mut_refs)]
pub fn get_backbuffer_fb() -> Framebuffer {
    unsafe {
        let addr = BACKBUFFER_MEM.as_mut_ptr() as u64;
        // 1920 width * 4 bytes per pixel = 7680 stride bytes
        Framebuffer::new(addr, 1920 * 4, 1920, 1080)
    }
}

/// Get raw pointer to the backbuffer memory.
pub fn backbuffer_ptr() -> *mut u32 {
    unsafe { BACKBUFFER_MEM.as_mut_ptr() }
}

/// Copy backbuffer to screen (double buffering)
pub fn present() {
    if let Some(disp) = display() {
        unsafe {
            let src = BACKBUFFER_MEM.as_ptr();
            let dst = disp.base;
            let size = (disp.width * disp.height) as usize;
            core::ptr::copy_nonoverlapping(src, dst, size);
        }
    }
}

/// Clear backbuffer
pub fn clear_backbuffer(color: Color) {
    unsafe {
        BACKBUFFER_MEM = [color.0; BACKBUFFER_SIZE];
    }
}

/// Get backbuffer pixel
pub fn get_backbuffer_pixel(x: u32, y: u32) -> Color {
    if x < 1920 && y < 1080 {
        unsafe {
            Color(BACKBUFFER_MEM[(y * 1920 + x) as usize])
        }
    } else {
        Color::BLACK
    }
}

/// Put pixel to backbuffer
pub fn put_backbuffer_pixel(x: u32, y: u32, color: Color) {
    if x < 1920 && y < 1080 {
        unsafe {
            BACKBUFFER_MEM[(y * 1920 + x) as usize] = color.0;
        }
    }
}
