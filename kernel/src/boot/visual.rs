//! Early visual overlay — writes directly to GOP framebuffer.
//!
//! This is a minimal text overlay shown during the first seconds of boot,
//! before the desktop subsystem is up. It is intentionally simple: a fixed
//! 360-pixel-tall banner at the top of the screen with 18 rotating log rows.
//!
//! Safety: all framebuffer access goes through `boot_info` globals, which are
//! populated by the boot stub before this module is used. If those globals
//! are zero (no framebuffer), every call is a no-op.

use core::sync::atomic::{AtomicUsize, Ordering};
use crate::boot_info;
use crate::ui::font;

pub(crate) const COLOR_BANNER: u32 = 0xFF050A12;
pub(crate) const COLOR_HEADER: u32 = 0xFF58A6FF;
pub(crate) const COLOR_OK:     u32 = 0xFF76B900;
pub(crate) const COLOR_WARN:   u32 = 0xFFFFBD2E;
pub(crate) const COLOR_FAULT:  u32 = 0xFFFF2A2A;
pub(crate) const COLOR_TEXT:   u32 = 0xFFE6EDF3;

const VISIBLE_ROWS: usize = 36;  // v1.5.1: 36 filas para no superponer
const ROW_HEIGHT: usize = 18;
const TOP_OFFSET: usize = 12;
const MAX_HEIGHT: usize = 700;  // cubre media pantalla

static EARLY_VISUAL_ROW: AtomicUsize = AtomicUsize::new(0);

/// Clear the boot banner area to a dark background.
pub fn clear() {
    let (addr, w, h, s) = unsafe {
        (
            boot_info::FB_ADDR,
            boot_info::FB_WIDTH as usize,
            boot_info::FB_HEIGHT as usize,
            boot_info::FB_STRIDE as usize,
        )
    };
    if addr == 0 || w == 0 || h == 0 || s == 0 { return; }

    let buf = addr as *mut u32;
    let max_h = h.min(MAX_HEIGHT);
    for y in 0..max_h {
        for x in 0..w {
            unsafe { buf.add(y * s + x).write_volatile(COLOR_BANNER); }
        }
    }
    EARLY_VISUAL_ROW.store(0, Ordering::Relaxed);
}

/// Log a boot message to the overlay. The row index rotates through 0..VISIBLE_ROWS.
pub fn log(phase: &str, msg: &str, color: u32) {
    let row = EARLY_VISUAL_ROW.fetch_add(1, Ordering::Relaxed) % VISIBLE_ROWS;
    let y = TOP_OFFSET + row * ROW_HEIGHT;
    text(12, y, b"FastOS KERNEL NEW :: ", COLOR_HEADER);
    text(188, y, phase.as_bytes(), color);
    text(188 + phase.len() * 8 + 16, y, msg.as_bytes(), COLOR_TEXT);
}

/// Write a 8x16 bitmap text glyph row at (x, y) in the given color.
pub fn text(x: usize, y: usize, text: &[u8], color: u32) {
    let (addr, w, h, s) = unsafe {
        (
            boot_info::FB_ADDR,
            boot_info::FB_WIDTH as usize,
            boot_info::FB_HEIGHT as usize,
            boot_info::FB_STRIDE as usize,
        )
    };
    if addr == 0 || w == 0 || h == 0 || s == 0 { return; }

    let mut cx = x;
    let buf = addr as *mut u32;
    for &ch in text {
        if cx + 8 >= w || y + 16 >= h { break; }
        let glyph = font::get_glyph(ch);
        for py in 0..16 {
            let bits = glyph[py];
            for px in 0..8 {
                if (bits & (0x80 >> px)) != 0 {
                    unsafe { buf.add((y + py) * s + cx + px).write_volatile(color); }
                }
            }
        }
        cx += 8;
    }
}

// Re-exports for backwards-compatible call sites that still import
// the free functions from `crate::main`. New code should use the
// `boot::visual` path directly.
#[allow(deprecated)]
pub use self::clear as early_visual_clear;
#[allow(deprecated)]
pub use self::log  as early_visual_log;
#[allow(deprecated)]
pub use self::text as early_visual_text;

pub mod color {
    pub(crate) use super::COLOR_OK as OK;
    pub(crate) use super::COLOR_WARN as WARN;
    pub(crate) use super::COLOR_FAULT as FAULT;
    pub(crate) use super::COLOR_HEADER as HEADER;
    pub(crate) use super::COLOR_TEXT as TEXT;
}
