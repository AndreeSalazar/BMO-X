//! Desktop — kernel-side helpers que sirven a los syscalls del compositor
//! Ring 3 (FbFill 0x61, FbText 0x62, KeyPoll 0x70).
//!
//! Mantiene cero estado dinámico: lee el framebuffer desde
//! `boot_info::FB_*`, escribe pixeles XRGB-8888 con `write_volatile` y
//! consulta el teclado PS/2 (`0x60` data / `0x64` status) sin bloquear.

#![allow(dead_code)]

use crate::boot_info;
use crate::font;

pub mod compositor;

// ────────────────────────────────────────────────────────────────────
// Pixel primitives
// ────────────────────────────────────────────────────────────────────

#[inline(always)]
fn fb_base() -> Option<(*mut u32, usize, usize, usize)> {
    let (addr, w, h, s) = unsafe {
        (boot_info::FB_ADDR, boot_info::FB_WIDTH as usize,
         boot_info::FB_HEIGHT as usize, boot_info::FB_STRIDE as usize)
    };
    if addr == 0 || w == 0 || h == 0 { return None; }
    Some((addr as *mut u32, s, w, h))
}

/// Llena un rectángulo del framebuffer con `color` (0xAARRGGBB).
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

/// Dibuja una cadena UTF-8 en el framebuffer (8×16, sin antialiasing).
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

// ────────────────────────────────────────────────────────────────────
// Input — PS/2 keyboard polling (no-blocking)
// ────────────────────────────────────────────────────────────────────

/// Devuelve el último scancode PS/2 disponible, o 0 si no hay tecla.
pub fn poll_key() -> u8 {
    let status: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16); }
    if (status & 1) == 0 { return 0; }
    let sc: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") sc, in("dx") 0x60u16); }
    sc
}
