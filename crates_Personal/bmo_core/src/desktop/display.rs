//! Display — Ring 0 framebuffer primitives for the desktop.
//!
//! Provides `fb_fill`, `fb_text`, `fb_blit` that the syscall handler (0x61/0x62/0x64)
//! and the render module use.

#![allow(dead_code)]

use crate::ui::font;

/// Raw framebuffer base pointer + dimensions (stride in pixels).
#[inline(always)]
pub(crate) fn fb_base() -> Option<(*mut u32, usize, usize, usize)> {
    let (addr, w, h, s) = unsafe {
        (crate::info::FB_ADDR, crate::info::FB_WIDTH as usize,
         crate::info::FB_HEIGHT as usize, crate::info::FB_STRIDE as usize)
    };
    if addr == 0 || w == 0 || h == 0 { return None; }
    Some((addr as *mut u32, s, w, h))
}

pub fn fb_fill(x: u32, y: u32, w: u32, h: u32, color: u32) {
    let Some((buf, stride, fbw, fbh)) = fb_base() else { return; };
    let x0 = (x as usize).min(fbw);
    let y0 = (y as usize).min(fbh);
    let x1 = ((x as usize) + (w as usize)).min(fbw);
    let y1 = ((y as usize) + (h as usize)).min(fbh);
    for row in y0..y1 {
        let line = unsafe { buf.add(row * stride) };
        for col in x0..x1 {
            unsafe { line.add(col).write_volatile(color); }
        }
    }
}

pub fn fb_text(x: u32, y: u32, text: &[u8], fg: u32) {
    let Some((buf, stride, fbw, fbh)) = fb_base() else { return; };
    let mut cx = x as usize;
    let cy = y as usize;
    for &ch in text {
        if cx + 8 > fbw { break; }
        if cy + 16 > fbh { break; }
        let glyph = font::get_glyph(ch);
        for py in 0..16 {
            let row = glyph[py];
            let line = unsafe { buf.add((cy + py) * stride) };
            for px in 0..8 {
                if (row & (0x80 >> px)) != 0 {
                    unsafe { line.add(cx + px).write_volatile(fg); }
                }
            }
        }
        cx += 8;
    }
}

pub fn fb_blit(x: u32, y: u32, w: u32, h: u32, src_ptr: u64) {
    let Some((buf, stride, fbw, fbh)) = fb_base() else { return; };
    if src_ptr == 0 || w == 0 || h == 0 { return; }
    let x0 = (x as usize).min(fbw);
    let y0 = (y as usize).min(fbh);
    let w = (w as usize).min(fbw.saturating_sub(x0));
    let h = (h as usize).min(fbh.saturating_sub(y0));
    let src = src_ptr as *const u32;
    for row in 0..h {
        let dst_line = unsafe { buf.add((y0 + row) * stride + x0) };
        let src_line = unsafe { src.add(row * w) };
        for col in 0..w {
            unsafe { dst_line.add(col).write_volatile(src_line.add(col).read()); }
        }
    }
}
