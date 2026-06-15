//! Overlay visual de diag/ sobre framebuffer GOP.

use super::buffer;
use super::event::{severity_color, severity_tag, Event};
use crate::boot_info;
use crate::font;

const OVERLAY_LINES: usize = 10;
const OVERLAY_W: usize = 960;
const OVERLAY_H: usize = 18 + OVERLAY_LINES * 18 + 10;

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
    if width < 320 || height < 220 { return; }

    let x = 16usize;
    let y = height.saturating_sub(OVERLAY_H + 16);
    let w = OVERLAY_W.min(width.saturating_sub(x + 1));
    let h = OVERLAY_H.min(height.saturating_sub(y + 1));

    // Semi-transparent dark background (glassmorphism style)
    fill_rect(base, stride, width, height, x, y, w, h, 0xEE0B1224);
    // Neon Cyan border
    draw_rect(base, stride, width, height, x, y, w, h, 0xFF56D4DD);
    // Subtle inner border for premium look
    draw_rect(base, stride, width, height, x + 1, y + 1, w - 2, h - 2, 0x3356D4DD);

    // Header text in neon green
    draw_text(base, stride, width, height, x + 12, y + 6, b"FastOS System Diagnostics HUD", 0xFF76B900);
    // Draw status indicators in header
    draw_text(base, stride, width, height, x + w - 180, y + 6, b"Mode: Ring 0 Supervisor", 0xFF8B949E);

    // Vertical divider
    fill_rect(base, stride, width, height, x + 310, y + 24, 1, h - 34, 0x3356D4DD);

    // ── LEFT COLUMN: System Monitor ──
    let lx = x + 12;
    let mut ly = y + 28;

    // CPU Info
    draw_text(base, stride, width, height, lx, ly, b"CPU    : Ryzen 5 5600X (Zen 3)", 0xFFE6EDF3);
    ly += 18;
    
    // CPU Active Features
    draw_text(base, stride, width, height, lx, ly, b"Exts   : SSE | AVX | AVX2 | FMA3", 0xFF56D4DD);
    ly += 18;

    // Uptime
    let uptime = crate::desktop::state::uptime_sec();
    let hours = uptime / 3600;
    let minutes = (uptime % 3600) / 60;
    let seconds = uptime % 60;
    let uptime_str = alloc::format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
    draw_text_str(base, stride, width, height, lx, ly, &alloc::format!("Uptime : {}", uptime_str), 0xFFE6EDF3);
    ly += 18;

    // Memory usage
    let free_pages = unsafe { crate::arch::page_alloc::free_count() };
    let free_mb = (free_pages * 4) / 1024;
    let mem_str = alloc::format!("Memory : {} MB free ({} p)", free_mb, free_pages);
    draw_text_str(base, stride, width, height, lx, ly, &mem_str, 0xFFE6EDF3);
    ly += 18;

    // Heap usage
    let heap_used = crate::allocator::heap_used();
    let heap_str = alloc::format!("Heap   : {} KB / 16384 KB", heap_used / 1024);
    let heap_color = if heap_used > 12 * 1024 * 1024 { 0xFFFF7B72 } else { 0xFF76B900 };
    draw_text_str(base, stride, width, height, lx, ly, &heap_str, heap_color);
    ly += 18;

    // Sched Info
    let p_count = crate::sched::process::process_count();
    let t_count = crate::sched::thread::ready_count();
    let sched_str = alloc::format!("Tasks  : Proc: {} | Thread: {}", p_count, t_count);
    draw_text_str(base, stride, width, height, lx, ly, &sched_str, 0xFFE6EDF3);
    ly += 18;

    // GOP Screen Details
    let scr_str = alloc::format!("GOP    : {}x{} (S:{})", width, height, stride);
    draw_text_str(base, stride, width, height, lx, ly, &scr_str, 0xFFE6EDF3);
    ly += 18;

    // Framebuffer Address
    let fb_addr = unsafe { boot_info::FB_ADDR };
    let fb_str = alloc::format!("FB Addr: 0x{:08X}", fb_addr);
    draw_text_str(base, stride, width, height, lx, ly, &fb_str, 0xFF8B949E);
    ly += 18;

    // Keyboard Toggle Hint
    draw_text(base, stride, width, height, lx, y + h - 18, b"Toggle: CTRL + ALT", 0xFFFFBD2E);

    // ── RIGHT COLUMN: Live Log Buffer ──
    let rx = x + 322;
    let mut ry = y + 28;
    let next = buffer::next_seq();
    let first = next.saturating_sub(OVERLAY_LINES as u64);
    let mut seq = first;
    while seq < next {
        if let Some(ev) = buffer::event_by_seq(seq) {
            draw_event_line(base, stride, width, height, rx, ry, ev);
            ry += 18;
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
    draw_text(base, stride, width, height, x + 144, y, b": ", 0xFFE6EDF3);
    
    // Truncate message if it is too long for the overlay right column
    let max_chars = (OVERLAY_W - 322 - 144 - 16) / 8;
    let msg_str = if ev.message.len() > max_chars {
        alloc::format!("{}...", &ev.message[..max_chars.saturating_sub(3)])
    } else {
        alloc::string::String::from(ev.message)
    };
    draw_text_str(base, stride, width, height, x + 160, y, &msg_str, 0xFFE6EDF3);

    if ev.has_value {
        draw_text(base, stride, width, height, x + OVERLAY_W - 322 - 160, y, b"0x", 0xFF8B949E);
        draw_hex(base, stride, width, height, x + OVERLAY_W - 322 - 144, y, ev.value, 0xFF8B949E);
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
