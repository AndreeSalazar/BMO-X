//! Overlay visual de diag/ sobre framebuffer GOP.
//!
//! Regla importante: este HUD corre dentro del camino de diagnóstico y puede
//! pintarse durante boot, panic o render del escritorio. Por eso no usa
//! `alloc::format!`, `String` ni helpers que crezcan el heap.

use super::buffer;
use super::event::{severity_color, severity_tag, Event};
use crate::boot_info;
use crate::ui::font;

const OVERLAY_LINES: usize = 7;
const MIN_W: usize = 360;
const MIN_H: usize = 220;
const MAX_W: usize = 1040;
const OVERLAY_H: usize = 252;
const CHAR_W: usize = 8;

static mut ENABLED: bool = true;

pub fn set_enabled(enabled: bool) {
    unsafe { ENABLED = enabled; }
}

pub fn is_enabled() -> bool {
    unsafe { ENABLED }
}

pub fn paint() {
    if unsafe { !ENABLED } { return; }

    let Some((base, width, height, stride)) = fb() else { return; };
    if width < MIN_W || height < MIN_H { return; }

    let x = 16usize.min(width.saturating_sub(1));
    let y = height.saturating_sub(OVERLAY_H + 16);
    let w = MAX_W.min(width.saturating_sub(x + 1)).max(MIN_W.min(width));
    let h = OVERLAY_H.min(height.saturating_sub(y + 1));
    if w < MIN_W || h < MIN_H { return; }

    // Fondo oscuro sólido: GOP no mezcla alpha, así que usamos color XRGB real.
    fill_rect(base, stride, width, height, x, y, w, h, 0xFF0B1224);
    draw_rect(base, stride, width, height, x, y, w, h, 0xFF56D4DD);
    if w > 4 && h > 4 {
        draw_rect(base, stride, width, height, x + 1, y + 1, w - 2, h - 2, 0xFF14344A);
    }

    draw_text(base, stride, width, height, x + 12, y + 6, b"FastOS diag/ HUD", 0xFF76B900);
    draw_text_right(base, stride, width, height, x + w - 12, y + 6, b"Ctrl+Alt: ocultar/ver", 0xFFFFBD2E);

    let mid = if w >= 720 { x + (w / 2) } else { x + 12 };
    if w >= 720 {
        fill_rect(base, stride, width, height, mid, y + 26, 1, h.saturating_sub(38), 0xFF14344A);
    }

    draw_left_column(base, stride, width, height, x + 12, y + 30);

    if w >= 720 {
        draw_right_column(base, stride, width, height, mid + 14, y + 30, x + w - 12);
    } else {
        draw_logs(base, stride, width, height, x + 12, y + 156, x + w - 12, OVERLAY_LINES.min(4));
    }
}

fn draw_left_column(base: *mut u32, stride: usize, width: usize, height: usize, x: usize, y: usize) {
    let mut cy = y;

    draw_text(base, stride, width, height, x, cy, b"CPU    : Ryzen 5 5600X (Zen 3)", 0xFFE6EDF3);
    cy += 18;
    draw_text(base, stride, width, height, x, cy, b"Exts   : SSE | AVX | AVX2 | FMA3", 0xFF56D4DD);
    cy += 18;
    draw_text(base, stride, width, height, x, cy, b"Ring   : 0 Supervisor | Ring3 preparado", 0xFFE6EDF3);
    cy += 18;

    let uptime = crate::desktop::state::uptime_sec();
    draw_text(base, stride, width, height, x, cy, b"Uptime : ", 0xFFE6EDF3);
    draw_two_digits(base, stride, width, height, x + 72, cy, uptime / 3600, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 88, cy, b":", 0xFFE6EDF3);
    draw_two_digits(base, stride, width, height, x + 96, cy, (uptime % 3600) / 60, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 112, cy, b":", 0xFFE6EDF3);
    draw_two_digits(base, stride, width, height, x + 120, cy, uptime % 60, 0xFFE6EDF3);
    cy += 18;

    let free_pages = unsafe { crate::arch::page_alloc::free_count() };
    draw_text(base, stride, width, height, x, cy, b"Memory : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 72, cy, (free_pages * 4 / 1024) as u64, 0xFF76B900);
    draw_text(base, stride, width, height, x + 128, cy, b" MB free", 0xFFE6EDF3);
    cy += 18;

    let heap_used_kb = crate::allocator::heap_used() / 1024;
    let heap_total_kb = crate::allocator::heap_total() / 1024;
    let heap_color = if heap_used_kb > heap_total_kb * 3 / 4 { 0xFFFF7B72 } else { 0xFF76B900 };
    draw_text(base, stride, width, height, x, cy, b"Heap   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 72, cy, heap_used_kb as u64, heap_color);
    draw_text(base, stride, width, height, x + 128, cy, b" / ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 160, cy, heap_total_kb as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 208, cy, b" KB", 0xFFE6EDF3);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Tasks  : P", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 88, cy, crate::sched::process::process_count() as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 120, cy, b" T", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 144, cy, crate::sched::thread::ready_count() as u64, 0xFFE6EDF3);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"GOP    : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 72, cy, width as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 112, cy, b"x", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, height as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 168, cy, b" stride ", 0xFF8B949E);
    draw_dec(base, stride, width, height, x + 232, cy, stride as u64, 0xFF8B949E);
}

fn draw_right_column(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    right: usize,
) {
    let mut cy = y;
    draw_text(base, stride, width, height, x, cy, b"BMO/FastOS live status", 0xFF58A6FF);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"PCI    : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 72, cy, crate::drivers::pci::device_count() as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 104, cy, b" devices", 0xFFE6EDF3);
    cy += 18;

    draw_bool_row(base, stride, width, height, x, cy, b"NVMe   : ", crate::drivers::pci::has_nvme());
    cy += 18;
    draw_bool_row(base, stride, width, height, x, cy, b"AHCI   : ", crate::drivers::pci::has_ahci());
    cy += 18;
    draw_bool_row(base, stride, width, height, x, cy, b"xHCI   : ", crate::drivers::pci::has_xhci());
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Storage: diferido seguro (boot estable)", 0xFFFFBD2E);
    cy += 18;
    draw_text(base, stride, width, height, x, cy, b"BareX  : GOP-first, driver real futuro", 0xFF8B949E);
    cy += 22;

    draw_text(base, stride, width, height, x, cy, b"diag log:", 0xFF76B900);
    draw_logs(base, stride, width, height, x, cy + 18, right, OVERLAY_LINES);
}

fn draw_bool_row(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    label: &[u8],
    value: bool,
) {
    draw_text(base, stride, width, height, x, y, label, 0xFFE6EDF3);
    if value {
        draw_text(base, stride, width, height, x + 72, y, b"detectado", 0xFF76B900);
    } else {
        draw_text(base, stride, width, height, x + 72, y, b"no activo", 0xFF8B949E);
    }
}

fn draw_logs(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    right: usize,
    lines: usize,
) {
    let next = buffer::next_seq();
    let first = next.saturating_sub(lines as u64);
    let mut seq = first;
    let mut cy = y;
    while seq < next {
        if let Some(ev) = buffer::event_by_seq(seq) {
            draw_event_line(base, stride, width, height, x, cy, right, ev);
            cy += 18;
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
    right: usize,
    ev: Event,
) {
    let color = severity_color(ev.severity);
    draw_text(base, stride, width, height, x, y, b"[", 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 8, y, severity_tag(ev.severity), color);
    draw_text(base, stride, width, height, x + 48, y, b"] ", 0xFFE6EDF3);
    draw_text_clipped(base, stride, width, height, x + 64, y, ev.module.as_bytes(), 0xFF76B900, 8);
    draw_text(base, stride, width, height, x + 128, y, b": ", 0xFFE6EDF3);
    let msg_x = x + 144;
    let value_cols = if ev.has_value { 19 } else { 0 };
    let max_cols = right.saturating_sub(msg_x).saturating_div(CHAR_W).saturating_sub(value_cols);
    draw_text_clipped(base, stride, width, height, msg_x, y, ev.message.as_bytes(), 0xFFE6EDF3, max_cols);

    if ev.has_value && right > 18 * CHAR_W {
        let vx = right.saturating_sub(18 * CHAR_W);
        draw_text(base, stride, width, height, vx, y, b"0x", 0xFF8B949E);
        draw_hex(base, stride, width, height, vx + 16, y, ev.value, 0xFF8B949E);
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
    let x1 = x.saturating_add(w).min(width);
    let y1 = y.saturating_add(h).min(height);
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
    if w == 0 || h == 0 { return; }
    fill_rect(base, stride, width, height, x, y, w, 1, color);
    fill_rect(base, stride, width, height, x, y + h.saturating_sub(1), w, 1, color);
    fill_rect(base, stride, width, height, x, y, 1, h, color);
    fill_rect(base, stride, width, height, x + w.saturating_sub(1), y, 1, h, color);
}

fn draw_text_right(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    right: usize,
    y: usize,
    text: &[u8],
    color: u32,
) {
    let px = text.len().saturating_mul(CHAR_W);
    draw_text(base, stride, width, height, right.saturating_sub(px), y, text, color);
}

fn draw_text_clipped(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    text: &[u8],
    color: u32,
    max_cols: usize,
) {
    let cols = text.len().min(max_cols);
    if cols == 0 { return; }
    draw_text(base, stride, width, height, x, y, &text[..cols], color);
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
        if cx + CHAR_W > width || y + 16 > height { break; }
        let glyph = font::get_glyph(ch);
        for gy in 0..16 {
            let row = glyph[gy];
            for gx in 0..8 {
                if (row & (0x80 >> gx)) != 0 {
                    unsafe { base.add((y + gy) * stride + cx + gx).write_volatile(color); }
                }
            }
        }
        cx += CHAR_W;
    }
}

fn draw_dec(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    mut value: u64,
    color: u32,
) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if value == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while value > 0 && i > 0 {
            i -= 1;
            buf[i] = b'0' + (value % 10) as u8;
            value /= 10;
        }
    }
    draw_text(base, stride, width, height, x, y, &buf[i..], color);
}

fn draw_two_digits(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    value: u64,
    color: u32,
) {
    let v = value % 100;
    let buf = [b'0' + (v / 10) as u8, b'0' + (v % 10) as u8];
    draw_text(base, stride, width, height, x, y, &buf, color);
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
    for (i, item) in buf.iter_mut().enumerate() {
        let shift = (15 - i) * 4;
        *item = hex[((value >> shift) & 0xF) as usize];
    }
    draw_text(base, stride, width, height, x, y, &buf, color);
}
