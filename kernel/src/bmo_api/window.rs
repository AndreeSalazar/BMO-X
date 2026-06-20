//! BMO API — Window struct + painting.
//!
//! Each window has a frame buffer region it owns. The window
//! struct stores the title, position, size, and a flag set.
//! The actual framebuffer write happens in `paint`.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> i32 { self.x + self.w }
    pub fn bottom(&self) -> i32 { self.y + self.h }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowFlags(pub u32);

impl WindowFlags {
    pub const NONE:      Self = Self(0);
    pub const VISIBLE:   Self = Self(1 << 0);
    pub const RESIZABLE: Self = Self(1 << 1);
    pub const ON_TOP:    Self = Self(1 << 2);
    pub const NO_CLOSE:  Self = Self(1 << 3);
    pub const NO_MOVE:   Self = Self(1 << 4);

    pub fn contains(self, other: Self) -> bool { self.0 & other.0 == other.0 }
}

#[derive(Debug, Clone)]
pub struct Window {
    pub id: u32,
    pub title: &'static str,
    pub rect: Rect,
    pub flags: WindowFlags,
    pub z_order: i32,
    pub dirty: bool,
    pub age_ticks: u32,
    /// Tick handler — called every ~16 ms while the window is
    /// focused. v1.6.22: optional. Most windows leave this None.
    pub on_tick: Option<fn(&mut Window)>,
}

impl Window {
    pub fn new(id: u32, title: &'static str, rect: Rect) -> Self {
        Self {
            id,
            title,
            rect,
            flags: WindowFlags::VISIBLE,
            z_order: 0,
            dirty: true,
            age_ticks: 0,
            on_tick: None,
        }
    }

    pub const TITLE_BAR_H: i32 = 28;

    pub fn title_bar_rect(&self) -> Rect {
        Rect::new(self.rect.x, self.rect.y, self.rect.w, Self::TITLE_BAR_H)
    }

    pub fn client_rect(&self) -> Rect {
        Rect::new(
            self.rect.x,
            self.rect.y + Self::TITLE_BAR_H,
            self.rect.w,
            self.rect.h - Self::TITLE_BAR_H,
        )
    }

    pub fn close_button_rect(&self) -> Rect {
        Rect::new(
            self.rect.x + self.rect.w - 28,
            self.rect.y + 4,
            24,
            Self::TITLE_BAR_H - 8,
        )
    }

    /// Paint the window frame: drop shadow, body, border, title bar,
    /// close button. Client area is filled with the dark client bg;
    /// the window's own `paint_client` callback (v1.7.x) draws on top.
    pub fn paint(&self, focused: bool) {
        use crate::desktop::display;

        let w = self.rect.w as u32;
        let h = self.rect.h as u32;
        let x = self.rect.x;
        let y = self.rect.y;
        let bg = 0xFF0F1827u32;
        let border_dim = 0xFF1F4D5Cu32;
        let border_bright = 0xFF4ECCA3u32;
        let title_bg = 0xFF1A2638u32;

        // 1) Drop shadow (8 px offset, very dark teal)
        if x + 12 < 1920 && y + 16 < 1080 {
            display::fb_fill((x + 12) as u32, (y + 16) as u32, w, h, 0xFF020610);
        }

        // 2) Window body
        display::fb_fill(x as u32, y as u32, w, h, bg);

        // 3) Border (mint if focused, dim teal otherwise)
        let bd = if focused { border_bright } else { border_dim };
        display::fb_fill(x as u32, y as u32, w, 2, bd);
        display::fb_fill(x as u32, (y + h as i32 - 2) as u32, w, 2, bd);
        display::fb_fill(x as u32, y as u32, 2, h, bd);
        display::fb_fill((x + w as i32 - 2) as u32, y as u32, 2, h, bd);

        // 4) Title bar
        let tbar = self.title_bar_rect();
        display::fb_fill(
            tbar.x as u32, tbar.y as u32,
            tbar.w as u32, tbar.h as u32,
            title_bg,
        );
        // mint accent under title bar
        display::fb_fill(
            tbar.x as u32, (tbar.y + tbar.h as i32 - 2) as u32,
            tbar.w as u32, 2, border_bright,
        );

        // 5) Title text (centered vertically in title bar)
        let title_y = (tbar.y + (tbar.h - 16) / 2) as u32;
        let title_x = (tbar.x + 12) as u32;
        let title_bytes = self.title.as_bytes();
        display::fb_text(title_x, title_y, title_bytes, 0xFFE6F1F5);

        // 6) Close button (X glyph) if not NO_CLOSE
        if !self.flags.contains(WindowFlags::NO_CLOSE) {
            let cr = self.close_button_rect();
            display::fb_fill(
                cr.x as u32, cr.y as u32,
                cr.w as u32, cr.h as u32,
                0xFF3A1B0E,
            );
            // X drawn as two diagonal lines, each 12 px
            // (using fill_rect with slope is complex; we use 2 thin lines)
            let cx0 = cr.x + 4;
            let cy0 = cr.y + 4;
            for i in 0..12 {
                // diagonal: \  (top-left to bottom-right)
                display::fb_fill((cx0 + i) as u32, (cy0 + i) as u32, 2, 1, 0xFFFF7B72);
                // diagonal: /  (top-right to bottom-left)
                display::fb_fill((cx0 + 11 - i) as u32, (cy0 + i) as u32, 2, 1, 0xFFFF7B72);
            }
        }
    }
}
