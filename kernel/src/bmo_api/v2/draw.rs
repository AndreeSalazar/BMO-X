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
pub fn fill_rect(_dc_slot: u32, _x: i32, _y: i32, _w: i32, _h: i32, _color: u32) {
    // v2.0: implementación real se conecta a gop::fill_rect con
    // clipping. Aquí dejamos el stub para no acoplar este módulo al
    // resto del kernel antes de tiempo.
}

pub fn draw_text(_dc_slot: u32, _x: i32, _y: i32, _text: &[u8], _color: u32) {}

pub fn draw_line(_dc_slot: u32, _x0: i32, _y0: i32, _x1: i32, _y1: i32, _color: u32) {}

/// Calcula el cliente rect de una ventana (área sin NC).
pub fn client_rect(w: &BmoWindow) -> (i32, i32, i32, i32) {
    let title_h = if w.style & super::window::style::WS_CAPTION != 0 { 28 } else { 0 };
    let cx = w.x;
    let cy = w.y + title_h;
    let cw = w.w;
    let ch = w.h - title_h;
    (cx, cy, cw, ch)
}
