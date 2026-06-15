#![allow(dead_code)]

//! Console — text display over framebuffer with scroll and cursor.

use crate::fb::{colors, Framebuffer};
use crate::font;

const CHAR_W: usize = 8;
const CHAR_H: usize = 16;
const SCROLLBACK_ROWS: usize = 8192;

#[repr(C)]
#[derive(Clone, Copy)]
struct HistoryCell {
    ch: u8,
    guard: u8,
    _pad: [u8; 2],
    fg: u32,
    bg: u32,
}

/// Console state.
pub struct Console {
    col: usize,
    row: usize,
    max_cols: usize,
    max_rows: usize,
    fg: u32,
    bg: u32,
    fb_addr: usize,
    fb_pitch: usize,  // bytes per scanline
    fb_stride: usize, // pixels per scanline (GOP stride)
    fb_width: usize,
    fb_height: usize,
    
    // Shadow buffer for double buffering (optional)
    shadow_addr: usize,
    shadow_pitch: usize,
    shadow_stride: usize,
    history_addr: usize,
    history_cols: usize,
    history_rows: usize,
    history_line: usize,
    view_offset: usize,
}

impl Console {
    pub fn new(fb_addr: u64, fb_pitch: u64, fb_stride: u32, fb_width: u32, fb_height: u32) -> Self {
        let fb_pitch = fb_pitch as usize;
        let fb_stride = fb_stride as usize;
        let fb_width = fb_width as usize;
        let fb_height = fb_height as usize;
        
        // Try to allocate shadow buffer (double buffering)
        let shadow_size = fb_pitch.saturating_mul(fb_height);
        let shadow_pages = (shadow_size + 0xFFF) / 0x1000;
        let shadow_addr =
            unsafe { crate::arch::page_alloc::alloc_pages_contiguous(shadow_pages).unwrap_or(0) }
                as usize;
        // Zero the shadow buffer to prevent garbage pixels
        if shadow_addr != 0 {
            unsafe {
                core::ptr::write_bytes(shadow_addr as *mut u8, 0, shadow_pages * 0x1000);
            }
        }

        let max_cols = fb_width / CHAR_W;
        let max_rows = fb_height / CHAR_H;

        let history_cols = max_cols;
        let history_rows = SCROLLBACK_ROWS;
        let history_size = core::mem::size_of::<HistoryCell>()
            .saturating_mul(history_cols)
            .saturating_mul(history_rows);
        let history_pages = (history_size + 0xFFF) / 0x1000;
        let history_addr =
            unsafe { crate::arch::page_alloc::alloc_pages_contiguous(history_pages).unwrap_or(0) }
                as usize;
        // Zero the history buffer
        if history_addr != 0 {
            unsafe {
                core::ptr::write_bytes(history_addr as *mut u8, 0, history_pages * 0x1000);
            }
        }

        Self {
            col: 0,
            row: 0,
            max_cols,
            max_rows,
            fg: colors::TEXT_PRIMARY,
            bg: colors::BG_DARK,
            fb_addr: fb_addr as usize,
            fb_pitch,
            fb_stride,
            fb_width,
            fb_height,
            shadow_addr,
            shadow_pitch: fb_pitch,
            shadow_stride: fb_stride,
            history_addr,
            history_cols,
            history_rows,
            history_line: 1,
            view_offset: 0,
        }
    }

    pub fn is_double_buffered(&self) -> bool {
        self.shadow_addr != 0
    }

    pub fn set_color(&mut self, fg: u32) {
        self.fg = fg;
    }

    pub fn col_pos(&self) -> usize {
        self.col
    }

    pub fn row_pos(&self) -> usize {
        self.row
    }

    pub fn set_pos(&mut self, col: usize, row: usize) {
        self.col = col.min(self.max_cols.saturating_sub(1));
        self.row = row.min(self.max_rows.saturating_sub(1));
    }

    pub fn set_colors(&mut self, fg: u32, bg: u32) {
        self.fg = fg;
        self.bg = bg;
    }

    pub fn clear(&mut self) {
        if self.shadow_addr != 0 {
            self.clear_shadow(self.bg);
            self.gradient_h_shadow(
                0,
                0,
                self.fb_width,
                2,
                colors::NV_GREEN,
                colors::ACCENT_CYAN,
            );
            self.flush_all();
        } else {
            let fb = self.fb();
            fb.clear(self.bg);

            // Top accent line
            fb.gradient_h(
                0,
                0,
                self.fb_width,
                2,
                colors::NV_GREEN,
                colors::ACCENT_CYAN,
            );
        }

        self.col = 0;
        self.row = 1; // Start below accent line
        self.history_line = 1;
        self.view_offset = 0;
        self.clear_history();
        self.clear_history_line(self.history_line);
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
                for _ in 0..spaces {
                    self.put_char(b' ');
                }
            }
            _ => {
                if self.row >= self.max_rows {
                    self.scroll();
                }
                self.view_offset = 0;
                self.store_history_cell(self.history_line, self.col, ch, self.fg, self.bg);
                self.draw_char(self.col, self.row, ch, self.fg, self.bg);
                self.flush_cell(self.col, self.row);
                self.col += 1;
                if self.col >= self.max_cols {
                    self.newline();
                }
            }
        }
    }

    pub fn newline(&mut self) {
        self.clear_line_tail(self.history_line, self.col, self.row);
        self.col = 0;
        self.history_line = self.history_line.saturating_add(1);
        self.clear_history_line(self.history_line);
        self.row += 1;
        if self.row >= self.max_rows {
            self.scroll();
        }
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.store_history_cell(self.history_line, self.col, b' ', self.fg, self.bg);
            self.draw_char(self.col, self.row, b' ', self.fg, self.bg);
            self.flush_cell(self.col, self.row);
        }
    }

    /// Print a u64 as decimal.
    pub fn print_u64(&mut self, mut val: u64) {
        if val == 0 {
            self.put_char(b'0');
            return;
        }
        let mut buf = [0u8; 20];
        let mut i = 0;
        while val > 0 {
            buf[i] = b'0' + (val % 10) as u8;
            val /= 10;
            i += 1;
        }
        while i > 0 {
            i -= 1;
            self.put_char(buf[i]);
        }
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
        let ch = if n < 10 {
            b'0' + n as u8
        } else {
            b'A' + (n as u8 - 10)
        };
        self.put_char(ch);
    }

    /// Framebuffer base address.
    pub fn fb_addr(&self) -> usize {
        self.fb_addr
    }

    /// Framebuffer pitch (bytes per scanline).
    pub fn fb_pitch(&self) -> usize {
        self.fb_pitch
    }

    // ── Internal ────────────────────────────────────────────────────────

    /// Framebuffer width in pixels.
    pub fn fb_width(&self) -> usize {
        self.fb_width
    }

    /// Framebuffer height in pixels.
    pub fn fb_height(&self) -> usize {
        self.fb_height
    }

    /// Scroll back through saved console output.
    pub fn scroll_back_lines(&mut self, lines: usize) {
        let live_top = self.live_top_history_line();
        let earliest = self.earliest_history_line();
        let max_offset = live_top.saturating_sub(earliest);
        self.view_offset = core::cmp::min(self.view_offset.saturating_add(lines), max_offset);
        self.render_history_view();
    }

    /// Scroll forward toward the live prompt.
    pub fn scroll_forward_lines(&mut self, lines: usize) {
        self.view_offset = self.view_offset.saturating_sub(lines);
        self.render_history_view();
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll_back_lines(self.max_rows.saturating_sub(2).max(1));
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_forward_lines(self.max_rows.saturating_sub(2).max(1));
    }

    pub fn scroll_to_top(&mut self) {
        let live_top = self.live_top_history_line();
        let earliest = self.earliest_history_line();
        self.view_offset = live_top.saturating_sub(earliest);
        self.render_history_view();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.view_offset = 0;
        self.render_history_view();
    }

    pub fn is_viewing_history(&self) -> bool {
        self.view_offset != 0
    }

    fn fb(&self) -> Framebuffer {
        Framebuffer::new(
            self.fb_addr as u64,
            self.fb_pitch as u64,
            self.fb_width as u32,
            self.fb_height as u32,
        )
    }

    fn draw_target(&self) -> (*mut u32, usize, bool) {
        if self.shadow_addr != 0 {
            (self.shadow_addr as *mut u32, self.shadow_stride, false)
        } else {
            (self.fb_addr as *mut u32, self.fb_stride, true)
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

    fn fill_rect_pixels(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let copy_w = w.min(self.fb_width.saturating_sub(x));
        let copy_h = h.min(self.fb_height.saturating_sub(y));
        if copy_w == 0 || copy_h == 0 {
            return;
        }

        let (buf, pitch_px, volatile) = self.draw_target();
        for row in 0..copy_h {
            for col in 0..copy_w {
                unsafe {
                    if volatile {
                        buf.add((y + row) * pitch_px + x + col).write_volatile(color);
                    } else {
                        buf.add((y + row) * pitch_px + x + col).write(color);
                    }
                }
            }
        }
        self.flush_rect(x, y, copy_w, copy_h);
    }

    fn history_enabled(&self) -> bool {
        self.history_addr != 0 && self.history_cols != 0
    }

    fn history_ptr(&self, abs_row: usize, col: usize) -> *mut HistoryCell {
        let row = abs_row % self.history_rows;
        (self.history_addr as *mut HistoryCell).wrapping_add(row * self.history_cols + col)
    }

    fn store_history_cell(&self, abs_row: usize, col: usize, ch: u8, fg: u32, bg: u32) {
        if !self.history_enabled() || col >= self.history_cols {
            return;
        }
        unsafe {
            self.history_ptr(abs_row, col).write(HistoryCell {
                ch,
                guard: Self::history_guard(ch, fg, bg),
                _pad: [0; 2],
                fg,
                bg,
            });
        }
    }

    fn read_history_cell(&self, abs_row: usize, col: usize) -> HistoryCell {
        let blank = self.blank_history_cell();
        if !self.history_enabled() || col >= self.history_cols {
            return blank;
        }
        unsafe {
            let cell = self.history_ptr(abs_row, col).read();
            if cell.ch == 0 {
                return blank;
            }
            if cell.guard != Self::history_guard(cell.ch, cell.fg, cell.bg) {
                return blank;
            }
            if cell.ch < b' ' || cell.ch > b'~' {
                return blank;
            }
            cell
        }
    }

    fn blank_history_cell(&self) -> HistoryCell {
        let ch = b' ';
        HistoryCell {
            ch,
            guard: Self::history_guard(ch, self.fg, self.bg),
            _pad: [0; 2],
            fg: self.fg,
            bg: self.bg,
        }
    }

    fn history_guard(ch: u8, fg: u32, bg: u32) -> u8 {
        ch ^ (fg as u8) ^ ((fg >> 16) as u8) ^ ((bg >> 8) as u8) ^ 0xA5
    }

    fn clear_line_tail(&self, abs_row: usize, col: usize, screen_row: usize) {
        if col >= self.max_cols || screen_row >= self.max_rows {
            return;
        }

        let mut c = col;
        while c < self.history_cols {
            self.store_history_cell(abs_row, c, b' ', self.fg, self.bg);
            c += 1;
        }

        self.fill_rect_pixels(
            col * CHAR_W,
            screen_row * CHAR_H,
            self.fb_width.saturating_sub(col * CHAR_W),
            CHAR_H,
            self.bg,
        );
    }

    fn clear_history(&self) {
        if self.history_addr == 0 {
            return;
        }
        let bytes = core::mem::size_of::<HistoryCell>()
            .saturating_mul(self.history_cols)
            .saturating_mul(self.history_rows);
        unsafe { core::ptr::write_bytes(self.history_addr as *mut u8, 0, bytes); }
    }

    fn clear_history_line(&self, abs_row: usize) {
        if !self.history_enabled() {
            return;
        }
        let row = abs_row % self.history_rows;
        unsafe {
            core::ptr::write_bytes(
                (self.history_addr as *mut HistoryCell).add(row * self.history_cols) as *mut u8,
                0,
                core::mem::size_of::<HistoryCell>() * self.history_cols,
            );
        }
    }

    fn live_top_history_line(&self) -> usize {
        self.history_line.saturating_add(1).saturating_sub(self.max_rows)
    }

    fn earliest_history_line(&self) -> usize {
        self.history_line.saturating_add(1).saturating_sub(self.history_rows)
    }

    fn render_history_view(&self) {
        if !self.history_enabled() {
            return;
        }

        if self.shadow_addr != 0 {
            self.clear_shadow(self.bg);
            self.gradient_h_shadow(0, 0, self.fb_width, 2, colors::NV_GREEN, colors::ACCENT_CYAN);
        } else {
            let fb = self.fb();
            fb.clear(self.bg);
            fb.gradient_h(0, 0, self.fb_width, 2, colors::NV_GREEN, colors::ACCENT_CYAN);
        }

        let top = self.live_top_history_line().saturating_sub(self.view_offset);
        let rows = self.max_rows;
        let cols = core::cmp::min(self.max_cols, self.history_cols);
        for screen_row in 0..rows {
            let abs_row = top.saturating_add(screen_row);
            for col in 0..cols {
                let cell = self.read_history_cell(abs_row, col);
                if cell.ch != b' ' || cell.bg != self.bg {
                    self.draw_char(col, screen_row, cell.ch, cell.fg, cell.bg);
                }
            }
        }
        if self.shadow_addr != 0 {
            self.flush_all();
        }
    }

    fn scroll(&mut self) {
        if self.shadow_addr != 0 {
            let buf = self.shadow_addr as *mut u32;
            let stride = self.shadow_stride;
            let copy_rows = (self.max_rows - 1) * CHAR_H;
            unsafe {
                core::ptr::copy(buf.add(CHAR_H * stride), buf, copy_rows * stride);
            }

            let bg = self.bg;
            let last_y = copy_rows;
            for py in 0..CHAR_H {
                for px in 0..self.fb_width {
                    unsafe {
                        buf.add((last_y + py) * stride + px).write(bg);
                    }
                }
            }

            self.row = self.max_rows - 1;
            self.flush_all();
            return;
        }

        let buf = self.fb_addr as *mut u32;
        let stride = self.fb_stride;
        let copy_rows = (self.max_rows - 1) * CHAR_H;
        for py in 0..copy_rows {
            for px in 0..self.fb_width {
                unsafe {
                    let src = buf.add((py + CHAR_H) * stride + px).read_volatile();
                    buf.add(py * stride + px).write_volatile(src);
                }
            }
        }
        let bg = self.bg;
        let last_y = copy_rows;
        for py in 0..CHAR_H {
            for px in 0..self.fb_width {
                unsafe {
                    buf.add((last_y + py) * stride + px).write_volatile(bg);
                }
            }
        }

        self.row = self.max_rows - 1;
    }

    fn clear_shadow(&self, color: u32) {
        let buf = self.shadow_addr as *mut u32;
        let stride = self.shadow_stride;
        for y in 0..self.fb_height {
            for x in 0..self.fb_width {
                unsafe {
                    buf.add(y * stride + x).write(color);
                }
            }
        }
    }

    fn gradient_h_shadow(&self, x: usize, y: usize, w: usize, h: usize, left: u32, right: u32) {
        let buf = self.shadow_addr as *mut u32;
        let stride = self.shadow_stride;
        for col in 0..w {
            let t = col as u32;
            let inv = (w - 1).max(1) as u32;
            let color = Self::lerp_color(left, right, t, inv);
            for row in y..(y + h).min(self.fb_height) {
                if x + col < self.fb_width {
                    unsafe {
                        buf.add(row * stride + x + col).write(color);
                    }
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
        if self.shadow_addr == 0 {
            return;
        }

        let copy_w = w.min(self.fb_width.saturating_sub(x));
        let copy_h = h.min(self.fb_height.saturating_sub(y));
        let src_stride = self.shadow_stride;
        let dst_stride = self.fb_stride;
        let src = self.shadow_addr as *const u32;
        let dst = self.fb_addr as *mut u32;

        for row in 0..copy_h {
            for col in 0..copy_w {
                unsafe {
                    let px = src.add((y + row) * src_stride + x + col).read();
                    dst.add((y + row) * dst_stride + x + col)
                        .write_volatile(px);
                }
            }
        }
    }

    pub fn flush_all(&self) {
        if self.shadow_addr == 0 {
            return;
        }

        let src_stride = self.shadow_stride;
        let dst_stride = self.fb_stride;
        let src = self.shadow_addr as *const u32;
        let dst = self.fb_addr as *mut u32;

        for y in 0..self.fb_height {
            for x in 0..self.fb_width {
                unsafe {
                    let px = src.add(y * src_stride + x).read();
                    dst.add(y * dst_stride + x).write_volatile(px);
                }
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
