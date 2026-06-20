//! v1.7.1 — Renderer del escritorio BMO (Ring 0). Mismo lenguaje visual
//! que el welcome: wallpaper procedural, glass cards, paleta dark elegante.
//!
//! `render_frame()` pinta un frame completo (wallpaper + status bar +
//! ventanas + dock + cursor) y `handle_input()` procesa el ratón (drag,
//! close-button, dock launcher).

#![allow(dead_code)]

use crate::boot_info;
use crate::bmo_core::ui::fb::Framebuffer;
use crate::bmo_core::ui::font;
use super::state::{self, DesktopState, DOCK_SLOTS, MAX_WIN, WinInfo};
use super::windows::{self as win, TITLES, DOCK_LABELS, DOCK_TO_TITLE};
use super::theme;
use super::wallpaper;

// ── Framebuffer helpers ────────────────────────────────────────────

fn fb() -> Option<Framebuffer> {
    let (addr, w, h, s) = unsafe {
        (boot_info::FB_ADDR, boot_info::FB_WIDTH, boot_info::FB_HEIGHT, boot_info::FB_STRIDE)
    };
    if addr == 0 || w == 0 { return None; }
    Some(Framebuffer::new(addr, (s as u64) * 4, w, h))
}

pub(crate) fn draw_text(fb: &Framebuffer, x: u32, y: u32, text: &[u8], color: u32) {
    let mut cx = x as usize;
    let cy = y as usize;
    for &ch in text {
        if cx + 8 > fb.width || cy + 16 > fb.height { break; }
        let glyph = font::get_glyph(ch);
        for py in 0..16 {
            let row = glyph[py];
            for px in 0..8 {
                if (row & (0x80 >> px)) != 0 {
                    fb.put_pixel(cx + px, cy + py, color);
                }
            }
        }
        cx += 8;
    }
}

fn fmt_hms(buf: &mut [u8; 8], h: u8, m: u8, s: u8) -> &str {
    buf[0] = b'0' + h / 10; buf[1] = b'0' + h % 10;
    buf[2] = b':';
    buf[3] = b'0' + m / 10; buf[4] = b'0' + m % 10;
    buf[5] = b':';
    buf[6] = b'0' + s / 10; buf[7] = b'0' + s % 10;
    core::str::from_utf8(buf).unwrap()
}

// ── Status bar ─────────────────────────────────────────────────────

#[allow(static_mut_refs)]
fn draw_status_bar(fb: &Framebuffer) {
    // Backdrop translúcido (glass dark).
    fb.fill_rect(0, 0, fb.width, 30, theme::GLASS_TINT);
    // Hairline inferior mint
    fb.fill_rect(0, 30, fb.width, 1, theme::MINT_DEEP);
    // Hairline superior blanco 6% (sheen)
    fb.fill_rect(0, 0, fb.width, 1, 0x14FFFFFF);

    draw_text(fb, 14, 7, b"\x95  FastOS", theme::MINT);
    draw_text(fb, 110, 7, b"File   Edit   View   Window   Help", theme::BODY);

    let (h, m, sec) = state::clock_hms();
    let mut buf = [0u8; 8];
    let clock_s = fmt_hms(&mut buf, h, m, sec);
    let st = unsafe { &state::STATE };

    let mut fbuf = [0u8; 48];
    fbuf[0..4].copy_from_slice(b"fps ");
    let mut p = 4;
    p += win::fmt_u64_into(&mut fbuf[p..], st.fps_display as u64);
    fbuf[p] = b' '; p += 1; fbuf[p] = b'|'; p += 1; fbuf[p] = b' '; p += 1;
    p += win::fmt_u64_into(&mut fbuf[p..], st.frame);
    let fps_str = &fbuf[..p];

    let fps_x = fb.width - (p * 8) - (8 * 12) - 16;
    draw_text(fb, fps_x as u32, 7, fps_str, theme::SUBTITLE);

    let clk_x = fb.width - 8 * 8 - 16;
    draw_text(fb, clk_x as u32, 7, clock_s.as_bytes(), theme::TITLE);
}

// ── Ventana ────────────────────────────────────────────────────────

#[allow(static_mut_refs)]
fn draw_window(fb: &Framebuffer, w: &WinInfo, active: bool) {
    let (x, y, ww, wh) = (w.x.max(0) as usize, w.y.max(0) as usize, w.w.max(0) as usize, w.h.max(0) as usize);
    if ww == 0 || wh == 0 { return; }

    // Sombra profunda
    fb.fill_rounded_rect(x + 8, y + 12, ww, wh, 16, theme::CARD_SHADOW);
    // Cuerpo glass
    fb.fill_rounded_rect(x, y, ww, wh, 16, theme::SURFACE_2);
    // Hairline interior
    fb.draw_rect(x + 1, y + 1, ww - 2, wh - 2, theme::SURFACE_LINE, 1);
    // Borde mint si activa, gris si no
    let bd = if active { theme::MINT } else { theme::SURFACE_BORDER };
    // Halo neón exterior (sólo activa)
    if active {
        fb.draw_rect(x.saturating_sub(2), y.saturating_sub(2), ww + 4, wh + 4, theme::NEON_INNER, 1);
    }
    fb.draw_rect(x, y, ww, wh, bd, if active { 2 } else { 1 });

    // Title bar
    let tb_color = if active { 0xFF123045 } else { 0xFF1A2535 };
    fb.fill_rounded_rect(x, y, ww, 36, 16, tb_color);
    // Sheen title bar
    fb.fill_rect(x + 1, y + 1, ww - 2, 1, theme::GLASS_HIGHLIGHT);
    // Línea inferior title bar
    fb.fill_rect(x + 1, y + 35, ww - 2, 1, 0xFF0A1A2A);

    // Traffic lights
    fb.fill_circle(x + 18, y + 18, 7, 0xFFFF5F56);
    fb.fill_circle(x + 38, y + 18, 7, 0xFFFFBD2E);
    fb.fill_circle(x + 58, y + 18, 7, 0xFF27C93F);

    // Título
    let title = TITLES[w.title_id as usize];
    let title_x = x + 80 + (ww.saturating_sub(80 + title.len() * 8 + 16)) / 2;
    draw_text(fb, title_x as u32, (y + 10) as u32, title, theme::TITLE);

    // Contenido
    let st = unsafe { &state::STATE };
    let mut buf1 = [0u8; 48];
    let mut buf2 = [0u8; 48];
    let lines = win::content_for(w.title_id, st.fps_display, st.frame, &mut buf1, &mut buf2);
    let mut cy = y + 52;
    for (line, color) in lines.iter() {
        if cy + 16 > y + wh { break; }
        draw_text(fb, (x + 18) as u32, cy as u32, line, *color);
        cy += 20;
    }
}

// ── Dock ───────────────────────────────────────────────────────────

const DOCK_ICON: usize = 56;
const DOCK_GAP: usize = 16;
const DOCK_PAD: usize = 14;

fn dock_geometry(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let inner_w = DOCK_SLOTS * DOCK_ICON + (DOCK_SLOTS - 1) * DOCK_GAP;
    let w = inner_w + 2 * DOCK_PAD;
    let h = DOCK_ICON + 2 * DOCK_PAD;
    let x = (fb.width - w) / 2;
    let y = fb.height - h - 18;
    (x, y, w, h)
}

fn icon_rect(fb: &Framebuffer, idx: usize) -> (usize, usize) {
    let (x, y, _, _) = dock_geometry(fb);
    let ix = x + DOCK_PAD + idx * (DOCK_ICON + DOCK_GAP);
    let iy = y + DOCK_PAD;
    (ix, iy)
}

// Acentos mint/violeta/cobalto/cyan/ámbar/rosa/gris
const DOCK_ACCENTS: [u32; 7] = [
    0xFF6C5CE7, // Terminal — violeta
    0xFF4ECCA3, // Editor — mint
    0xFF56D4DD, // Files — cyan
    0xFFE2C044, // Notes — gold
    0xFFE07832, // Tasks — orange
    0xFFFF7B72, // Monitor — rose
    0xFF7F848A, // Settings — gray
];

#[allow(static_mut_refs)]
fn draw_dock(fb: &Framebuffer) {
    let (dx, dy, dw, dh) = dock_geometry(fb);
    // sombra
    fb.fill_rounded_rect(dx + 4, dy + 8, dw, dh, 24, theme::CARD_SHADOW);
    // glass body
    fb.fill_rounded_rect(dx, dy, dw, dh, 24, theme::GLASS_TINT);
    // hairline
    fb.draw_rect(dx + 1, dy + 1, dw - 2, dh - 2, theme::SURFACE_LINE, 1);
    // border mint sutil
    fb.draw_rect(dx, dy, dw, dh, theme::MINT_DEEP, 1);
    // top sheen
    fb.fill_rect(dx + 4, dy + 2, dw - 8, 1, theme::GLASS_HIGHLIGHT);

    let st = unsafe { &state::STATE };
    let hover = st.dock_hover;

    for i in 0..DOCK_SLOTS {
        let (ix, iy) = icon_rect(fb, i);
        if hover == i as i32 {
            // halo hover: pill con mint 18% alpha simulando glass hover
            fb.fill_rounded_rect(ix - 8, iy - 8, DOCK_ICON + 16, DOCK_ICON + 16, 14, 0x304ECCA3);
            fb.draw_rect(ix - 8, iy - 8, DOCK_ICON + 16, DOCK_ICON + 16, theme::MINT, 1);
        }
        // Icono con gradiente simulado (cuadrado con un highlight)
        fb.fill_rounded_rect(ix, iy, DOCK_ICON, DOCK_ICON, 12, DOCK_ACCENTS[i]);
        // top sheen del icono
        fb.fill_rect(ix + 2, iy + 2, DOCK_ICON - 4, 1, 0x33FFFFFF);
        // borde interno
        fb.draw_rect(ix, iy, DOCK_ICON, DOCK_ICON, 0x33000000, 1);

        // Indicador "abierta": dot blanco
        let mut is_open = false;
        for j in 0..MAX_WIN {
            if st.windows[j].open && st.windows[j].title_id == DOCK_TO_TITLE[i] {
                is_open = true; break;
            }
        }
        if is_open {
            let cx = ix + DOCK_ICON / 2;
            let cy = iy + DOCK_ICON + 8;
            fb.fill_circle(cx, cy, 3, theme::MINT_SOFT);
        }
    }

    if hover >= 0 {
        let label = DOCK_LABELS[hover as usize];
        let (ix, iy) = icon_rect(fb, hover as usize);
        let lx = ix + DOCK_ICON / 2 - (label.len() * 8) / 2;
        let ly = iy.saturating_sub(28);
        // tooltip pill
        fb.fill_rounded_rect(lx.saturating_sub(10), ly.saturating_sub(4),
                             label.len() * 8 + 20, 22, 8, theme::SURFACE_0);
        fb.draw_rect(lx.saturating_sub(10), ly.saturating_sub(4),
                     label.len() * 8 + 20, 22, theme::MINT_DEEP, 1);
        draw_text(fb, lx as u32, ly as u32, label, theme::TITLE);
    }
}

// ── Cursor ─────────────────────────────────────────────────────────

const CURSOR: [&[u8]; 17] = [
    b"X           ", b"XX          ", b"XOX         ", b"XOOX        ",
    b"XOOOX       ", b"XOOOOX      ", b"XOOOOOX     ", b"XOOOOOOX    ",
    b"XOOOOOOOX   ", b"XOOOOOOOOX  ", b"XOOOOOXXXXX ", b"XOOXOOX     ",
    b"XOX XOOX    ", b"XX  XOOX    ", b"     XOOX   ", b"      XOOX  ",
    b"       XXX  ",
];

fn draw_cursor(fb: &Framebuffer, x: i32, y: i32) {
    if x < 0 || y < 0 { return; }
    for (row, line) in CURSOR.iter().enumerate() {
        for (col, ch) in line.iter().enumerate() {
            let px = (x as usize) + col;
            let py = (y as usize) + row;
            match *ch {
                b'X' => fb.put_pixel(px, py, theme::CARD_SHADOW),
                b'O' => fb.put_pixel(px, py, theme::TITLE),
                _ => {}
            }
        }
    }
}

// ── Input handling — drag, close, dock launcher ────────────────────

fn point_in_rect(px: i32, py: i32, x: i32, y: i32, w: i32, h: i32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn point_in_circle(px: i32, py: i32, cx: i32, cy: i32, r: i32) -> bool {
    let dx = px - cx; let dy = py - cy;
    dx * dx + dy * dy <= r * r
}

#[allow(static_mut_refs)]
fn handle_input(fb: &Framebuffer) {
    let st: &mut DesktopState = unsafe { &mut state::STATE };
    let mx = st.mouse_x; let my = st.mouse_y;

    if st.drag_idx >= 0 {
        if state::mouse_left_held() {
            let idx = st.drag_idx as usize;
            if idx < MAX_WIN && st.windows[idx].open {
                let new_x = mx - st.drag_dx;
                let new_y = my - st.drag_dy;
                let maxx = (fb.width as i32) - st.windows[idx].w;
                let maxy = (fb.height as i32) - st.windows[idx].h;
                let clamped_x = new_x.clamp(0, maxx.max(0));
                let clamped_y = new_y.clamp(30, maxy.max(30));
                if clamped_x != st.windows[idx].x || clamped_y != st.windows[idx].y {
                    st.windows[idx].x = clamped_x;
                    st.windows[idx].y = clamped_y;
                    unsafe { state::DIRTY = true; }
                }
            }
        } else {
            st.drag_idx = -1;
            unsafe { state::DIRTY = true; }
        }
        return;
    }

    let mut hover: i32 = -1;
    for i in 0..DOCK_SLOTS {
        let (ix, iy) = icon_rect(fb, i);
        if mx as usize >= ix && (mx as usize) < ix + DOCK_ICON &&
           my as usize >= iy && (my as usize) < iy + DOCK_ICON {
            hover = i as i32;
        }
    }
    if st.dock_hover != hover {
        st.dock_hover = hover;
        unsafe { state::DIRTY = true; }
    }

    if !state::mouse_left_pressed() { return; }

    if hover >= 0 {
        state::open_window(DOCK_TO_TITLE[hover as usize]);
        if st.dock_active != hover {
            st.dock_active = hover;
            unsafe { state::DIRTY = true; }
        }
        return;
    }

    let order = z_order_top_first();
    for &idx in order.iter() {
        let w = st.windows[idx];
        if !w.open { continue; }

        if point_in_circle(mx, my, w.x + 18, w.y + 16, 9) {
            state::close_window(idx);
            return;
        }

        if point_in_rect(mx, my, w.x + 80, w.y, w.w - 80, 32) ||
           point_in_rect(mx, my, w.x, w.y, w.w, 32) && !point_in_circle(mx, my, w.x + 18, w.y + 16, 9)
                                                    && !point_in_circle(mx, my, w.x + 38, w.y + 16, 9)
                                                    && !point_in_circle(mx, my, w.x + 58, w.y + 16, 9) {
            if st.focus != idx as i32 {
                st.focus = idx as i32;
                unsafe { state::DIRTY = true; }
            }
            st.drag_idx = idx as i32;
            st.drag_dx = mx - w.x;
            st.drag_dy = my - w.y;
            unsafe { state::DIRTY = true; }
            return;
        }

        if point_in_rect(mx, my, w.x, w.y, w.w, w.h) {
            if st.focus != idx as i32 {
                st.focus = idx as i32;
                unsafe { state::DIRTY = true; }
            }
            return;
        }
    }
}

#[allow(static_mut_refs)]
fn z_order_top_first() -> [usize; MAX_WIN] {
    let st = unsafe { &state::STATE };
    let mut out = [0usize; MAX_WIN];
    let mut p = 0;
    if st.focus >= 0 && (st.focus as usize) < MAX_WIN {
        out[p] = st.focus as usize; p += 1;
    }
    for i in 0..MAX_WIN {
        if i as i32 == st.focus { continue; }
        if p < MAX_WIN { out[p] = i; p += 1; }
    }
    out
}

fn wait_for_vsync() {
    // No-op en UEFI puro.
}

// ── Frame ──────────────────────────────────────────────────────────

#[allow(static_mut_refs)]
pub fn render_frame() {
    state::tick();

    if !unsafe { state::DIRTY } {
        return;
    }
    unsafe { state::DIRTY = false; }

    let (addr, w, h, s) = unsafe {
        (boot_info::FB_ADDR, boot_info::FB_WIDTH, boot_info::FB_HEIGHT, boot_info::FB_STRIDE)
    };
    if addr == 0 || w == 0 {
        crate::bmo_core::diag::fault("desktop", "render_frame: FB_ADDR or width is zero");
        return;
    }

    let backbuffer_fb = crate::device::gop::get_backbuffer_fb();

    handle_input(&backbuffer_fb);

    // Wallpaper procedural compartido (mismo look que el welcome).
    let time = crate::cpu::rdtsc();
    wallpaper::draw(&backbuffer_fb, time);

    draw_status_bar(&backbuffer_fb);

    // Ventanas: focus al final (encima)
    let st = unsafe { &state::STATE };
    for i in 0..MAX_WIN {
        if i as i32 == st.focus { continue; }
        if st.windows[i].open {
            draw_window(&backbuffer_fb, &st.windows[i], false);
        }
    }
    if st.focus >= 0 && (st.focus as usize) < MAX_WIN {
        let w = st.windows[st.focus as usize];
        if w.open { draw_window(&backbuffer_fb, &w, true); }
    }

    draw_dock(&backbuffer_fb);

    draw_cursor(&backbuffer_fb, st.mouse_x, st.mouse_y);

    if crate::bmo_core::diag::is_overlay_enabled() {
        let bb_addr = crate::device::gop::backbuffer_ptr() as *mut u32;
        crate::bmo_core::diag::overlay::set_target_override(Some((bb_addr, w as usize, h as usize, s as usize)));
        crate::bmo_core::diag::paint_overlay();
        crate::bmo_core::diag::overlay::set_target_override(None);
    }

    wait_for_vsync();

    let screen_fb = Framebuffer::new(addr, (s as u64) * 4, w, h);
    backbuffer_fb.blit_to(&screen_fb);
}
