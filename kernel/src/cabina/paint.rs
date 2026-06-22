//! `cabina::paint` — Primitivas de dibujo sobre el framebuffer GOP.
//!
//! v1.8.8: usa la font 8x16 de `bmo_core::ui::font` y el framebuffer
//! GOP. Si el FB no está inicializado, los draws son no-op (van a
//! serial como fallback).
//!
//! ## API
//!
//! - `draw_text(x, y, s, color)` — dibuja texto 8x16.
//! - `fill_rect(x, y, w, h, color)` — rellena un rect.
//! - `text_width(s)` — ancho en píxeles de un texto.
//! - `text_height()` — alto de una línea (16).

#![allow(dead_code)]

use crate::bmo_core::ui::fb::Framebuffer;
use crate::bmo_core::ui::font;

/// Acceso lazy al framebuffer. Construye la primera vez.
static mut FB: Option<Framebuffer> = None;

/// Inicializa el framebuffer desde BOOT_INFO. Llamar desde `cabina::init`
/// después de que BOOT_INFO esté disponible.
pub fn init() {
    unsafe {
        if FB.is_some() { return; }
        let addr = crate::boot::info::FB_ADDR;
        let w = crate::boot::info::FB_WIDTH;
        let h = crate::boot::info::FB_HEIGHT;
        let s = crate::boot::info::FB_STRIDE;
        if addr != 0 && w != 0 && h != 0 {
            FB = Some(Framebuffer::new(addr, (s as u64) * 4, w, h));
        }
    }
}

/// Dibuja texto 8x16 en (x, y) con color. Si el FB no está
/// inicializado, va a serial como fallback.
pub fn draw_text(x: u32, y: u32, s: &str, color: u32) {
    unsafe {
        if let Some(fb) = FB.as_ref() {
            draw_text_fb(fb, x, y, s, color);
        } else {
            crate::dev::console::serial_write(&alloc::format!("[cabina] {}\n", s));
        }
    }
}

fn draw_text_fb(fb: &Framebuffer, x: u32, y: u32, s: &str, color: u32) {
    let mut cx = x;
    for byte in s.bytes() {
        let glyph = font::get_glyph(byte);
        draw_glyph(fb, cx, y, &glyph, color);
        cx += 8;
        if cx >= fb.width as u32 { break; }
    }
}

fn draw_glyph(fb: &Framebuffer, x: u32, y: u32, glyph: &[u8; 16], color: u32) {
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..8u32 {
            if (bits >> (7 - col)) & 1 != 0 {
                fb.put_pixel((x + col) as usize, (y + row as u32) as usize, color);
            }
        }
    }
}

/// Rellena un rectángulo con color.
pub fn fill_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    unsafe {
        if let Some(fb) = FB.as_ref() {
            fb.fill_rect(x as usize, y as usize, w as usize, h as usize, color);
        }
    }
}

/// Ancho de un texto en píxeles.
pub fn text_width(s: &str) -> u32 { s.len() as u32 * 8 }

/// Alto de una línea de texto en píxeles.
pub const fn text_height() -> u32 { 16 }

/// Líneas visibles en pantalla (asumiendo texto 16px).
pub fn visible_lines() -> u32 {
    unsafe {
        if let Some(fb) = FB.as_ref() { (fb.height as u32) / 16 } else { 50 }
    }
}
