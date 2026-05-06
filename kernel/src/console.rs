//! Console — text display over framebuffer with scroll and cursor.

use crate::fb::{Framebuffer, colors};
use crate::font;

const CHAR_W: usize = 8;
const CHAR_H: usize = 16;

/// Console state.
pub struct Console {
    col: usize,
    row: usize,
    max_cols: usize,
    max_rows: usize,
    fg: u32,
    bg: u32,
    fb_addr: usize,
    fb_pitch: usize,
    fb_width: usize,
    fb_height: usize,
    shadow_addr: usize,
    shadow_pitch: usize,
}

impl Console {
    pub fn new(fb_addr: u64, fb_pitch: u64, fb_width: u32, fb_height: u32) -> Self {
        let fb_pitch = fb_pitch as usize;
        let fb_width = fb_width as usize;
        let fb_height = fb_height as usize;
        let shadow_size = fb_pitch.saturating_mul(fb_height);
        let shadow_pages = (shadow_size + 0xFFF) / 0x1000;
        let shadow_addr = unsafe {
            crate::arch::page_alloc::alloc_pages_contiguous(shadow_pages)
                .unwrap_or(0)
        } as usize;

        if shadow_addr != 0 {
            unsafe { core::ptr::write_bytes(shadow_addr as *mut u8, 0, shadow_pages * 0x1000); }
        }

        Self {
            col: 0,
            row: 0,
            max_cols: fb_width / CHAR_W,
            max_rows: fb_height / CHAR_H,
            fg: colors::TEXT_PRIMARY,
            bg: colors::BG_DARK,
            fb_addr: fb_addr as usize,
            fb_pitch,
            fb_width,
            fb_height,
            shadow_addr,
            shadow_pitch: fb_pitch,
        }
    }

    pub fn is_double_buffered(&self) -> bool {
        self.shadow_addr != 0
    }

    pub fn set_color(&mut self, fg: u32) {
        self.fg = fg;
    }

    pub fn set_colors(&mut self, fg: u32, bg: u32) {
        self.fg = fg;
        self.bg = bg;
    }

    pub fn clear(&mut self) {
        if self.shadow_addr != 0 {
            self.clear_shadow(self.bg);
            self.gradient_h_shadow(0, 0, self.fb_width, 2, colors::NV_GREEN, colors::ACCENT_CYAN);
            self.flush_all();
        } else {
            let fb = self.fb();
            fb.clear(self.bg);

            // Top accent line
            fb.gradient_h(0, 0, self.fb_width, 2, colors::NV_GREEN, colors::ACCENT_CYAN);
        }

        self.col = 0;
        self.row = 1; // Start below accent line
    }

    pub fn print(&mut self, s: &str) {
        for b in s.bytes() {
            self.put_char(b);
        }
    }

    pub fn println(&mut self, s: &str) {
        self.print(s);
        self.newline();
    }

    pub fn print_colored(&mut self, s: &str, fg: u32) {
        let old = self.fg;
        self.fg = fg;
        self.print(s);
        self.fg = old;
    }

    pub fn put_char(&mut self, ch: u8) {
        match ch {
            b'\n' => self.newline(),
            8 => self.backspace(),
            b'\t' => {
                let spaces = 4 - (self.col % 4);
                for _ in 0..spaces { self.put_char(b' '); }
            }
            _ => {
                if self.row >= self.max_rows { self.scroll(); }
                self.draw_char(self.col, self.row, ch, self.fg, self.bg);
                self.flush_cell(self.col, self.row);
                self.col += 1;
                if self.col >= self.max_cols { self.newline(); }
            }
        }
    }

    pub fn newline(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= self.max_rows { self.scroll(); }
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.draw_char(self.col, self.row, b' ', self.fg, self.bg);
            self.flush_cell(self.col, self.row);
        }
    }

    /// Print a u64 as decimal.
    pub fn print_u64(&mut self, mut val: u64) {
        if val == 0 { self.put_char(b'0'); return; }
        let mut buf = [0u8; 20];
        let mut i = 0;
        while val > 0 { buf[i] = b'0' + (val % 10) as u8; val /= 10; i += 1; }
        while i > 0 { i -= 1; self.put_char(buf[i]); }
    }

    /// Print a u32 as hex.
    pub fn print_hex32(&mut self, val: u32) {
        self.print("0x");
        self.put_hex_nibble((val >> 28) & 0xF);
        self.put_hex_nibble((val >> 24) & 0xF);
        self.put_hex_nibble((val >> 20) & 0xF);
        self.put_hex_nibble((val >> 16) & 0xF);
        self.put_hex_nibble((val >> 12) & 0xF);
        self.put_hex_nibble((val >> 8) & 0xF);
        self.put_hex_nibble((val >> 4) & 0xF);
        self.put_hex_nibble(val & 0xF);
    }

    fn put_hex_nibble(&mut self, n: u32) {
        let ch = if n < 10 { b'0' + n as u8 } else { b'A' + (n as u8 - 10) };
        self.put_char(ch);
    }

    /// Framebuffer base address.
    pub fn fb_addr(&self) -> usize { self.fb_addr }

    /// Framebuffer pitch (bytes per scanline).
    pub fn fb_pitch(&self) -> usize { self.fb_pitch }

    // ── Internal ────────────────────────────────────────────────────────

    fn fb(&self) -> Framebuffer {
        Framebuffer::new(self.fb_addr as u64, self.fb_pitch as u64, self.fb_width as u32, self.fb_height as u32)
    }

    fn draw_target(&self) -> (*mut u32, usize, bool) {
        if self.shadow_addr != 0 {
            (self.shadow_addr as *mut u32, self.shadow_pitch / 4, false)
        } else {
            (self.fb_addr as *mut u32, self.fb_pitch / 4, true)
        }
    }

    fn draw_char(&self, col: usize, row: usize, ch: u8, fg: u32, bg: u32) {
        let glyph = font::get_glyph(ch);
        let base_x = col * CHAR_W;
        let base_y = row * CHAR_H;
        let (buf, pitch_px, volatile) = self.draw_target();

        for gy in 0..CHAR_H {
            let bits = glyph[gy];
            for gx in 0..CHAR_W {
                let px = if bits & (0x80 >> gx) != 0 { fg } else { bg };
                let off = (base_y + gy) * pitch_px + (base_x + gx);
                unsafe {
                    if volatile {
                        buf.add(off).write_volatile(px);
                    } else {
                        buf.add(off).write(px);
                    }
                }
            }
        }
    }

    fn scroll(&mut self) {
        if self.shadow_addr != 0 {
            let buf = self.shadow_addr as *mut u32;
            let pitch_px = self.shadow_pitch / 4;
            let copy_rows = (self.max_rows - 1) * CHAR_H;
            unsafe {
                core::ptr::copy(
                    buf.add(CHAR_H * pitch_px),
                    buf,
                    copy_rows * pitch_px,
                );
            }

            let bg = self.bg;
            let last_y = copy_rows;
            for py in 0..CHAR_H {
                for px in 0..self.fb_width {
                    unsafe { buf.add((last_y + py) * pitch_px + px).write(bg); }
                }
            }

            self.row = self.max_rows - 1;
            self.flush_all();
            return;
        }

        let buf = self.fb_addr as *mut u32;
        let pitch_px = self.fb_pitch / 4;
        let copy_rows = (self.max_rows - 1) * CHAR_H;
        for py in 0..copy_rows {
            for px in 0..self.fb_width {
                unsafe {
                    let src = buf.add((py + CHAR_H) * pitch_px + px).read_volatile();
                    buf.add(py * pitch_px + px).write_volatile(src);
                }
            }
        }
        let bg = self.bg;
        let last_y = copy_rows;
        for py in 0..CHAR_H {
            for px in 0..self.fb_width {
                unsafe { buf.add((last_y + py) * pitch_px + px).write_volatile(bg); }
            }
        }

        self.row = self.max_rows - 1;
    }

    fn clear_shadow(&self, color: u32) {
        let buf = self.shadow_addr as *mut u32;
        let pitch_px = self.shadow_pitch / 4;
        for y in 0..self.fb_height {
            for x in 0..self.fb_width {
                unsafe { buf.add(y * pitch_px + x).write(color); }
            }
        }
    }

    fn gradient_h_shadow(&self, x: usize, y: usize, w: usize, h: usize, left: u32, right: u32) {
        let buf = self.shadow_addr as *mut u32;
        let pitch_px = self.shadow_pitch / 4;
        for col in 0..w {
            let t = col as u32;
            let inv = (w - 1).max(1) as u32;
            let color = Self::lerp_color(left, right, t, inv);
            for row in y..(y + h).min(self.fb_height) {
                if x + col < self.fb_width {
                    unsafe { buf.add(row * pitch_px + x + col).write(color); }
                }
            }
        }
    }

    fn lerp_color(a: u32, b: u32, t: u32, total: u32) -> u32 {
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

    fn flush_cell(&self, col: usize, row: usize) {
        self.flush_rect(col * CHAR_W, row * CHAR_H, CHAR_W, CHAR_H);
    }

    fn flush_rect(&self, x: usize, y: usize, w: usize, h: usize) {
        if self.shadow_addr == 0 { return; }

        let copy_w = w.min(self.fb_width.saturating_sub(x));
        let copy_h = h.min(self.fb_height.saturating_sub(y));
        let src_pitch_px = self.shadow_pitch / 4;
        let dst_pitch_px = self.fb_pitch / 4;
        let src = self.shadow_addr as *const u32;
        let dst = self.fb_addr as *mut u32;

        for row in 0..copy_h {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    src.add((y + row) * src_pitch_px + x),
                    dst.add((y + row) * dst_pitch_px + x),
                    copy_w,
                );
            }
        }
    }

    pub fn flush_all(&self) {
        if self.shadow_addr == 0 { return; }

        let src_pitch_px = self.shadow_pitch / 4;
        let dst_pitch_px = self.fb_pitch / 4;
        let src = self.shadow_addr as *const u32;
        let dst = self.fb_addr as *mut u32;

        for y in 0..self.fb_height {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    src.add(y * src_pitch_px),
                    dst.add(y * dst_pitch_px),
                    self.fb_width,
                );
            }
        }
    }

    /// Draw a simple cursor block at current position.
    pub fn draw_cursor(&self, visible: bool) {
        let color = if visible { self.fg } else { self.bg };
        let x = self.col * CHAR_W;
        let y = self.row * CHAR_H + CHAR_H - 2;
        let (buf, pitch_px, volatile) = self.draw_target();
        for px in 0..CHAR_W {
            for py in 0..2usize {
                unsafe {
                    if volatile {
                        buf.add((y + py) * pitch_px + x + px).write_volatile(color);
                    } else {
                        buf.add((y + py) * pitch_px + x + px).write(color);
                    }
                }
            }
        }
        self.flush_rect(x, y, CHAR_W, 2);
    }
}
