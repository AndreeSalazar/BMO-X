//! v2.0 — DC (Device Context) y primitivas de dibujo.
//!
//! Las primitivas escriben en el framebuffer de vuelta (o en la
//! surface asignada al DC). En v2.0 simplificamos: cada DC tiene un
//! framebuffer lógico (su surface) y todas las primitivas lo tratan
//! como XRGB32 1920×1080.

#![allow(dead_code)]

use super::window::BmoWindow;

#[derive(Debug, Clone, Copy)]
pub struct BmoDc {
    pub used: bool,
    pub id: u32,
    pub generation: u16,
    pub owner_window: u32,
    pub clip_x: i32, pub clip_y: i32, pub clip_w: i32, pub clip_h: i32,
    pub text_color: u32,
    pub bg_color: u32,
    pub pen_color: u32,
    pub brush_color: u32,
    pub font_id: u8,
    pub target_surface: u32,
}

impl BmoDc {
    pub const fn empty() -> Self {
        Self {
            used: false, id: 0, generation: 0,
            owner_window: 0,
            clip_x: 0, clip_y: 0, clip_w: 0, clip_h: 0,
            text_color: 0xFFE6F1F5, bg_color: 0xFF0F1827,
            pen_color: 0xFFE6F1F5, brush_color: 0xFF1F4D5C,
            font_id: 0,
            target_surface: 0,
        }
    }

    pub fn clip_contains(&self, x: i32, y: i32) -> bool {
        x >= self.clip_x && y >= self.clip_y
            && x < self.clip_x + self.clip_w && y < self.clip_y + self.clip_h
    }
}

pub const MAX_DCS: usize = 256;

pub struct DcTable {
    pub dcs: [BmoDc; MAX_DCS],
    pub next_id: u32,
}

impl DcTable {
    pub const fn new() -> Self {
        const D: BmoDc = BmoDc::empty();
        Self { dcs: [D; MAX_DCS], next_id: 1 }
    }
}

static mut DC_TABLE: DcTable = DcTable::new();

pub fn dc_table() -> &'static mut DcTable {
    unsafe { &mut DC_TABLE }
}

/// Crea un DC para la ventana indicada. Devuelve el slot index.
pub fn create_dc_for(window_slot: u32) -> Option<u32> {
    let t = dc_table();
    for (i, d) in t.dcs.iter_mut().enumerate() {
        if !d.used {
            d.used = true;
            d.id = t.next_id;
            d.generation = d.generation.wrapping_add(1);
            d.owner_window = window_slot;
            d.clip_x = 0; d.clip_y = 0;
            d.clip_w = 1920; d.clip_h = 1080;
            d.text_color = 0xFFE6F1F5;
            d.bg_color = 0xFF0F1827;
            d.pen_color = 0xFFE6F1F5;
            d.brush_color = 0xFF1F4D5C;
            t.next_id = t.next_id.wrapping_add(1);
            return Some(i as u32);
        }
    }
    None
}

/// Primitivas — escriben en el framebuffer global. En v2.0 las
/// superficies de ventana son lógicas (no se blitean); el dibujado
/// va directamente al GOP framebuffer con clipping contra el rect
/// de la ventana dueña del DC.
pub fn fill_rect(dc_slot: u32, x: i32, y: i32, w: i32, h: i32, color: u32) {
    let dc = match dc_table().dcs.get(dc_slot as usize) {
        Some(d) if d.used => *d,
        _ => return,
    };
    let (cx, cy, cw, ch) = client_rect_from_clip(&dc);
    let ix = x.max(cx);
    let iy = y.max(cy);
    let ix2 = (x + w).min(cx + cw);
    let iy2 = (y + h).min(cy + ch);
    if ix >= ix2 || iy >= iy2 { return; }
    let fb = get_fb();
    fb.fill_rect(ix as usize, iy as usize, (ix2 - ix) as usize, (iy2 - iy) as usize, color);
}

pub fn draw_text(dc_slot: u32, x: i32, y: i32, text: &[u8], color: u32) {
    let dc = match dc_table().dcs.get(dc_slot as usize) {
        Some(d) if d.used => *d,
        _ => return,
    };
    let (cx, cy, cw, _ch) = client_rect_from_clip(&dc);
    let fb = get_fb();
    let mut cx_pos = x;
    for &byte in text {
        if cx_pos >= cx + cw { break; }
        let glyph = crate::bmo_core::ui::font::get_glyph(byte);
        for gy in 0..16 {
            let row = glyph[gy];
            for gx in 0..8 {
                if row & (0x80 >> gx) != 0 {
                    let px = cx_pos + gx;
                    let py = y + gy as i32;
                    if px >= cx && px < cx + cw && py >= cy && py < cy + _ch {
                        fb.put_pixel(px as usize, py as usize, color);
                    }
                }
            }
        }
        cx_pos += 8;
    }
}

pub fn draw_line(dc_slot: u32, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    let dc = match dc_table().dcs.get(dc_slot as usize) {
        Some(d) if d.used => *d,
        _ => return,
    };
    let (cx, cy, cw, ch) = client_rect_from_clip(&dc);
    let fb = get_fb();
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        if x >= cx && x < cx + cw && y >= cy && y < cy + ch {
            fb.put_pixel(x as usize, y as usize, color);
        }
        if x == x1 && y == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

fn get_fb() -> crate::bmo_core::ui::fb::Framebuffer {
    let (addr, stride, width, height) = unsafe {
        (crate::boot::info::FB_ADDR, crate::boot::info::FB_STRIDE, crate::boot::info::FB_WIDTH, crate::boot::info::FB_HEIGHT)
    };
    crate::bmo_core::ui::fb::Framebuffer::new(addr, stride as u64, width, height)
}

fn client_rect_from_clip(dc: &BmoDc) -> (i32, i32, i32, i32) {
    (dc.clip_x, dc.clip_y, dc.clip_w, dc.clip_h)
}

/// Calcula el cliente rect de una ventana (área sin NC).
pub fn client_rect(w: &BmoWindow) -> (i32, i32, i32, i32) {
    let title_h = if w.style & super::window::style::WS_CAPTION != 0 { 28 } else { 0 };
    let cx = w.x;
    let cy = w.y + title_h;
    let cw = w.w;
    let ch = w.h - title_h;
    (cx, cy, cw, ch)
}
