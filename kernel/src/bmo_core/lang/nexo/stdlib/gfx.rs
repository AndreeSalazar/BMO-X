//! ÑEXO std::gfx — Primitivas gráficas.

#![allow(dead_code)]

use crate::bmo_core::lang::nexo::runtime::io::Graphics;

pub fn clear(color: u32) { if let Some(g) = Graphics::new() { g.clear(color); } }
pub fn pixel(x: usize, y: usize, color: u32) { if let Some(g) = Graphics::new() { g.pixel(x, y, color); } }
pub fn rect(x: usize, y: usize, w: usize, h: usize, color: u32) { if let Some(g) = Graphics::new() { g.fill_rect(x, y, w, h, color); } }
pub fn text(x: usize, y: usize, s: &str, color: u32, scale: usize) { if let Some(g) = Graphics::new() { g.draw_text(x, y, s, color, scale); } }
pub fn screen_width() -> usize { unsafe { crate::boot_info::FB_WIDTH as usize } }
pub fn screen_height() -> usize { unsafe { crate::boot_info::FB_HEIGHT as usize } }
