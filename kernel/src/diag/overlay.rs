//! Overlay visual de diag/ sobre framebuffer GOP.

use super::buffer;
use super::event::{severity_color, severity_tag, Event};
use crate::boot_info;
use crate::font;

const OVERLAY_LINES: usize = 8;
const OVERLAY_W: usize = 760;
const OVERLAY_H: usize = 18 + OVERLAY_LINES * 18;

static mut ENABLED: bool = true;

pub fn set_enabled(enabled: bool) {
    unsafe { ENABLED = enabled; }
}

pub fn paint() {
    if unsafe { !ENABLED } { return; }

    let Some((base, width, height, stride)) = fb() else { return; };
    if width < 320 || height < 220 { return; }

    let x = 12usize;
    let y = height.saturating_sub(OVERLAY_H + 12);
    let w = OVERLAY_W.min(width.saturating_sub(x + 1));
    let h = OVERLAY_H.min(height.saturating_sub(y + 1));

    fill_rect(base, stride, width, height, x, y, w, h, 0xCC050810);
    draw_rect(base, stride, width, height, x, y, w, h, 0xFF56D4DD);
    draw_text(base, stride, width, height, x + 10, y + 6, b"FastOS diag/ live", 0xFFE6EDF3);

    let next = buffer::next_seq();
    let first = next.saturating_sub(OVERLAY_LINES as u64);
    let mut row = 0usize;
    let mut seq = first;
    while seq < next && row < OVERLAY_LINES {
        if let Some(ev) = buffer::event_by_seq(seq) {
            draw_event_line(base, stride, width, height, x + 10, y + 28 + row * 18, ev);
            row += 1;
        }
        seq += 1;
    }
}

fn draw_event_line(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    ev: Event,
) {
    let color = severity_color(ev.severity);
    draw_text(base, stride, width, height, x, y, b"[", 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 8, y, severity_tag(ev.severity), color);
    draw_text(base, stride, width, height, x + 48, y, b"] ", 0xFFE6EDF3);
    draw_text_str(base, stride, width, height, x + 64, y, ev.module, 0xFF76B900);
    draw_text(base, stride, width, height, x + 160, y, b": ", 0xFFE6EDF3);
    draw_text_str(base, stride, width, height, x + 176, y, ev.message, 0xFFE6EDF3);
    if ev.has_value {
        draw_text(base, stride, width, height, x + 600, y, b"0x", 0xFF8B949E);
        draw_hex(base, stride, width, height, x + 616, y, ev.value, 0xFF8B949E);
    }
}

fn fb() -> Option<(*mut u32, usize, usize, usize)> {
    let (addr, w, h, s) = unsafe {
        (
            boot_info::FB_ADDR,
            boot_info::FB_WIDTH as usize,
            boot_info::FB_HEIGHT as usize,
            boot_info::FB_STRIDE as usize,
        )
    };
    if addr == 0 || w == 0 || h == 0 || s == 0 { return None; }
    Some((addr as *mut u32, w, h, s))
}

fn fill_rect(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    let x1 = (x + w).min(width);
    let y1 = (y + h).min(height);
    for yy in y..y1 {
        for xx in x..x1 {
            unsafe { base.add(yy * stride + xx).write_volatile(color); }
        }
    }
}

fn draw_rect(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    fill_rect(base, stride, width, height, x, y, w, 1, color);
    fill_rect(base, stride, width, height, x, y + h.saturating_sub(1), w, 1, color);
    fill_rect(base, stride, width, height, x, y, 1, h, color);
    fill_rect(base, stride, width, height, x + w.saturating_sub(1), y, 1, h, color);
}

fn draw_text_str(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    text: &str,
    color: u32,
) {
    draw_text(base, stride, width, height, x, y, text.as_bytes(), color);
}

fn draw_text(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    text: &[u8],
    color: u32,
) {
    let mut cx = x;
    for &ch in text {
        if cx + 8 >= width || y + 16 >= height { break; }
        let glyph = font::get_glyph(ch);
        for gy in 0..16 {
            let row = glyph[gy];
            for gx in 0..8 {
                if (row & (0x80 >> gx)) != 0 {
                    unsafe { base.add((y + gy) * stride + cx + gx).write_volatile(color); }
                }
            }
        }
        cx += 8;
    }
}

fn draw_hex(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    value: u64,
    color: u32,
) {
    let hex = b"0123456789ABCDEF";
    let mut buf = [0u8; 16];
    for i in 0..16 {
        let shift = (15 - i) * 4;
        buf[i] = hex[((value >> shift) & 0xF) as usize];
    }
    draw_text(base, stride, width, height, x, y, &buf, color);
}
