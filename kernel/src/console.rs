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
}

impl Console {
    pub fn new(fb_addr: u64, fb_pitch: u64, fb_width: u32, fb_height: u32) -> Self {
        Self {
            col: 0,
            row: 0,
            max_cols: fb_width as usize / CHAR_W,
            max_rows: fb_height as usize / CHAR_H,
            fg: colors::TEXT_PRIMARY,
            bg: colors::BG_DARK,
            fb_addr: fb_addr as usize,
            fb_pitch: fb_pitch as usize,
            fb_width: fb_width as usize,
            fb_height: fb_height as usize,
        }
    }

    pub fn set_color(&mut self, fg: u32) {
        self.fg = fg;
    }

    pub fn set_colors(&mut self, fg: u32, bg: u32) {
        self.fg = fg;
        self.bg = bg;
    }

    pub fn clear(&mut self) {
        let fb = self.fb();
        fb.clear(self.bg);

        // Top accent line
        fb.gradient_h(0, 0, self.fb_width, 2, colors::NV_GREEN, colors::ACCENT_CYAN);

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

    fn draw_char(&self, col: usize, row: usize, ch: u8, fg: u32, bg: u32) {
        let glyph = font::get_glyph(ch);
        let base_x = col * CHAR_W;
        let base_y = row * CHAR_H;
        let buf = self.fb_addr as *mut u32;
        let pitch_px = self.fb_pitch / 4;

        for gy in 0..CHAR_H {
            let bits = glyph[gy];
            for gx in 0..CHAR_W {
                let px = if bits & (0x80 >> gx) != 0 { fg } else { bg };
                let off = (base_y + gy) * pitch_px + (base_x + gx);
                unsafe { buf.add(off).write_volatile(px); }
            }
        }
    }

    fn scroll(&mut self) {
        let buf = self.fb_addr as *mut u32;
        let pitch_px = self.fb_pitch / 4;

        // Copy all rows up by one character height
        let copy_rows = (self.max_rows - 1) * CHAR_H;
        for py in 0..copy_rows {
            for px in 0..self.fb_width {
                unsafe {
                    let src = buf.add((py + CHAR_H) * pitch_px + px).read_volatile();
                    buf.add(py * pitch_px + px).write_volatile(src);
                }
            }
        }

        // Clear bottom row
        let bg = self.bg;
        let last_y = copy_rows;
        for py in 0..CHAR_H {
            for px in 0..self.fb_width {
                unsafe { buf.add((last_y + py) * pitch_px + px).write_volatile(bg); }
            }
        }

        self.row = self.max_rows - 1;
    }

    /// Draw a simple cursor block at current position.
    pub fn draw_cursor(&self, visible: bool) {
        let color = if visible { self.fg } else { self.bg };
        let x = self.col * CHAR_W;
        let y = self.row * CHAR_H + CHAR_H - 2;
        let buf = self.fb_addr as *mut u32;
        let pitch_px = self.fb_pitch / 4;
        for px in 0..CHAR_W {
            for py in 0..2usize {
                unsafe { buf.add((y + py) * pitch_px + x + px).write_volatile(color); }
            }
        }
    }
}
