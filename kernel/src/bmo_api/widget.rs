//! BMO API — pre-built widgets.
//!
//! Widgets are simple drawing helpers for common UI elements.
//! v1.6.22: label, button, panel. v1.7.x will add textbox, listbox.

use crate::desktop::display;

/// Draw a left-aligned label with optional color override.
pub fn label(x: u32, y: u32, text: &[u8], color: u32) {
    display::fb_text(x, y, text, color);
}

/// Draw a centered label inside a horizontal rect.
pub fn label_centered(rect_x: u32, rect_w: u32, y: u32, text: &[u8], color: u32) {
    let text_w = text.len() as u32 * 8;
    let x = rect_x + (rect_w.saturating_sub(text_w)) / 2;
    display::fb_text(x, y, text, color);
}

/// Draw a button (mint pill with darker border).
pub fn button(x: u32, y: u32, w: u32, h: u32, label: &[u8]) {
    // Pill background
    display::fb_fill(x, y, w, h, 0xFF1A2638);
    // Mint border
    display::fb_fill(x, y, w, 2, 0xFF4ECCA3);
    display::fb_fill(x, y + h - 2, w, 2, 0xFF4ECCA3);
    display::fb_fill(x, y, 2, h, 0xFF4ECCA3);
    display::fb_fill(x + w - 2, y, 2, h, 0xFF4ECCA3);
    // Label centered
    label_centered(x, w, y + (h.saturating_sub(16)) / 2, label, 0xFFE6F1F5);
}

/// Draw a horizontal divider (1 px line) at y.
pub fn divider(x: u32, y: u32, w: u32) {
    display::fb_fill(x, y, w, 1, 0xFF1F4D5C);
}

/// Draw a panel (dark card with subtle border).
pub fn panel(x: u32, y: u32, w: u32, h: u32) {
    display::fb_fill(x, y, w, h, 0xFF1A2638);
    display::fb_fill(x, y, w, 1, 0xFF1F4D5C);                          // top
    display::fb_fill(x, y + h - 1, w, 1, 0xFF1F4D5C);                    // bottom
    display::fb_fill(x, y, 1, h, 0xFF1F4D5C);                          // left
    display::fb_fill(x + w - 1, y, 1, h, 0xFF1F4D5C);                    // right
}
