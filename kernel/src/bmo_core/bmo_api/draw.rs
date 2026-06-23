//! v3.0 — DC + primitivas de dibujo.
//!
//! Surface blit al framebuffer, draw_image, DC save/restore stack.

#![allow(dead_code)]

use super::window::BmoWindow;
use core::sync::atomic::{AtomicU8, Ordering};
use core::cell::UnsafeCell;

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
    pub save_count: u8,
}

const DC_SAVE_MAX: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct DcSaveEntry {
    clip_x: i32, clip_y: i32, clip_w: i32, clip_h: i32,
    text_color: u32, bg_color: u32, pen_color: u32, brush_color: u32,
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
            save_count: 0,
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
    pub saves: [[DcSaveEntry; DC_SAVE_MAX]; MAX_DCS],
}

impl DcTable {
    pub const fn new() -> Self {
        const D: BmoDc = BmoDc::empty();
        const S: DcSaveEntry = DcSaveEntry {
            clip_x: 0, clip_y: 0, clip_w: 0, clip_h: 0,
            text_color: 0, bg_color: 0, pen_color: 0, brush_color: 0,
        };
        Self { dcs: [D; MAX_DCS], next_id: 1, saves: [[S; DC_SAVE_MAX]; MAX_DCS] }
    }
}

pub struct DcTableLock {
    data: UnsafeCell<DcTable>,
    lock: AtomicU8,
}

impl DcTableLock {
    pub const fn new() -> Self {
        Self { data: UnsafeCell::new(DcTable::new()), lock: AtomicU8::new(0) }
    }

    pub fn acquire(&self) {
        loop {
            match self.lock.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => return,
                Err(_) => core::hint::spin_loop(),
            }
        }
    }
    pub fn release(&self) { self.lock.store(0, Ordering::Release); }

    pub unsafe fn get(&self) -> &mut DcTable {
        &mut *self.data.get()
    }
}

unsafe impl Sync for DcTableLock {}

pub static DC_TABLE_LOCK: DcTableLock = DcTableLock::new();

pub unsafe fn dc_table() -> &'static mut DcTable {
    &mut *DC_TABLE_LOCK.data.get()
}

pub fn create_dc_for(window_slot: u32) -> Option<u32> {
    DC_TABLE_LOCK.acquire();
    let t = unsafe { dc_table() };
    let r = {
        let mut found = None;
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
                d.save_count = 0;
                t.next_id = t.next_id.wrapping_add(1);
                found = Some(i as u32);
                break;
            }
        }
        found
    };
    DC_TABLE_LOCK.release();
    r
}

pub fn save_dc(dc_slot: u32) -> bool {
    DC_TABLE_LOCK.acquire();
    let t = unsafe { dc_table() };
    let ok = if let Some(d) = t.dcs.get(dc_slot as usize) {
        d.used && (d.save_count as usize) < DC_SAVE_MAX
    } else { false };
    if ok {
        let d = &t.dcs[dc_slot as usize];
        let s = DcSaveEntry {
            clip_x: d.clip_x, clip_y: d.clip_y, clip_w: d.clip_w, clip_h: d.clip_h,
            text_color: d.text_color, bg_color: d.bg_color,
            pen_color: d.pen_color, brush_color: d.brush_color,
        };
        let sc = d.save_count as usize;
        t.saves[dc_slot as usize][sc] = s;
        let d = &mut t.dcs[dc_slot as usize];
        d.save_count += 1;
    }
    DC_TABLE_LOCK.release();
    ok
}

pub fn restore_dc(dc_slot: u32) -> bool {
    DC_TABLE_LOCK.acquire();
    let t = unsafe { dc_table() };
    let ok = if let Some(d) = t.dcs.get(dc_slot as usize) {
        d.used && d.save_count > 0
    } else { false };
    if ok {
        let sc = (t.dcs[dc_slot as usize].save_count as usize) - 1;
        let s = t.saves[dc_slot as usize][sc];
        let d = &mut t.dcs[dc_slot as usize];
        d.clip_x = s.clip_x;
        d.clip_y = s.clip_y;
        d.clip_w = s.clip_w;
        d.clip_h = s.clip_h;
        d.text_color = s.text_color;
        d.bg_color = s.bg_color;
        d.pen_color = s.pen_color;
        d.brush_color = s.brush_color;
        d.save_count -= 1;
    }
    DC_TABLE_LOCK.release();
    ok
}

pub fn fill_rect(dc_slot: u32, x: i32, y: i32, w: i32, h: i32, color: u32) {
    DC_TABLE_LOCK.acquire();
    let dc = unsafe { dc_table().dcs.get(dc_slot as usize) }.and_then(|d| if d.used { Some(*d) } else { None });
    DC_TABLE_LOCK.release();
    let dc = match dc { Some(d) => d, None => return };
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
    DC_TABLE_LOCK.acquire();
    let dc = unsafe { dc_table().dcs.get(dc_slot as usize) }.and_then(|d| if d.used { Some(*d) } else { None });
    DC_TABLE_LOCK.release();
    let dc = match dc { Some(d) => d, None => return };
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
    DC_TABLE_LOCK.acquire();
    let dc = unsafe { dc_table().dcs.get(dc_slot as usize) }.and_then(|d| if d.used { Some(*d) } else { None });
    DC_TABLE_LOCK.release();
    let dc = match dc { Some(d) => d, None => return };
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

pub fn draw_pixel(dc_slot: u32, x: i32, y: i32, color: u32) {
    DC_TABLE_LOCK.acquire();
    let dc = unsafe { dc_table().dcs.get(dc_slot as usize) }.and_then(|d| if d.used { Some(*d) } else { None });
    DC_TABLE_LOCK.release();
    let dc = match dc { Some(d) => d, None => return };
    let (cx, cy, cw, ch) = client_rect_from_clip(&dc);
    if x >= cx && x < cx + cw && y >= cy && y < cy + ch {
        get_fb().put_pixel(x as usize, y as usize, color);
    }
}

pub fn draw_rect(dc_slot: u32, x: i32, y: i32, w: i32, h: i32, color: u32) {
    draw_line(dc_slot, x, y, x + w, y, color);
    draw_line(dc_slot, x + w, y, x + w, y + h, color);
    draw_line(dc_slot, x + w, y + h, x, y + h, color);
    draw_line(dc_slot, x, y + h, x, y, color);
}

pub fn draw_image(dc_slot: u32, dst_x: i32, dst_y: i32, src_pixels: *const u32, src_w: u32, src_h: u32, src_pitch: u32) {
    if src_pixels.is_null() || src_w == 0 || src_h == 0 { return; }
    DC_TABLE_LOCK.acquire();
    let dc = unsafe { dc_table().dcs.get(dc_slot as usize) }.and_then(|d| if d.used { Some(*d) } else { None });
    DC_TABLE_LOCK.release();
    let dc = match dc { Some(d) => d, None => return };
    let (cx, cy, cw, ch) = client_rect_from_clip(&dc);
    let fb = get_fb();
    let src = unsafe { core::slice::from_raw_parts(src_pixels, (src_pitch as usize / 4) * src_h as usize) };
    for sy in 0..src_h {
        let py = dst_y + sy as i32;
        if py < cy || py >= cy + ch { continue; }
        for sx in 0..src_w {
            let px = dst_x + sx as i32;
            if px < cx || px >= cx + cw { continue; }
            let sp = (sy as usize) * (src_pitch as usize / 4) + sx as usize;
            if sp < src.len() {
                let color = src[sp];
                if color & 0xFF000000 != 0 {
                    fb.put_pixel(px as usize, py as usize, color);
                }
            }
        }
    }
}

pub fn blit_surface_to_fb(surface_slot: u32, dst_x: i32, dst_y: i32) {
    let st = super::surface::surface_table();
    st.acquire();
    let info = st.surface(surface_slot).map(|srf| {
        (srf.pixels, srf.width as u32, srf.height as u32, srf.pitch as u32)
    });
    st.release();
    let (pixels, sw, sh, pitch) = match info {
        Some(i) => i,
        None => return,
    };
    if pixels.is_null() || sw == 0 || sh == 0 { return; }
    let fb = get_fb();
    let (fbw, fbh) = unsafe { (crate::boot::info::FB_WIDTH, crate::boot::info::FB_HEIGHT) };
    let pixels = unsafe { core::slice::from_raw_parts(pixels, (pitch as usize / 4) * sh as usize) };
    for sy in 0..sh {
        let py = dst_y + sy as i32;
        if py < 0 || py >= fbh as i32 { continue; }
        for sx in 0..sw {
            let px = dst_x + sx as i32;
            if px < 0 || px >= fbw as i32 { continue; }
            let sp = sy as usize * (pitch as usize / 4) + sx as usize;
            if sp < pixels.len() {
                let color = pixels[sp];
                fb.put_pixel(px as usize, py as usize, color);
            }
        }
    }
}

fn get_fb() -> crate::bmo_core::ui::fb::Framebuffer {
    let (addr, stride, width, height) = unsafe {
        (crate::boot::info::FB_ADDR, crate::boot::info::FB_STRIDE, crate::boot::info::FB_WIDTH, crate::boot::info::FB_HEIGHT)
    };
    crate::bmo_core::ui::fb::Framebuffer::new(addr, (stride as u64) * 4, width, height)
}

fn client_rect_from_clip(dc: &BmoDc) -> (i32, i32, i32, i32) {
    (dc.clip_x, dc.clip_y, dc.clip_w, dc.clip_h)
}

pub fn client_rect(w: &BmoWindow) -> (i32, i32, i32, i32) {
    w.client_rect()
}
