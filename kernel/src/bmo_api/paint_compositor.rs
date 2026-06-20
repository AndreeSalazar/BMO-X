//! v2.0 — Compositor de pintado.
//!
//! Recorre la Z-list, encuentra ventanas dirty, dibuja su contenido
//! en la surface (o directamente en el framebuffer en v2.0) y limpia
//! el flag de dirty. En una implementación completa usaría blit desde
//! la offscreen surface al framebuffer; aquí pintamos directo.

#![allow(dead_code)]

use super::window::WID_INVALID;
#[allow(unused_imports)]
use super::wm;
use crate::drivers::gop;
use crate::desktop::theme;

static mut LAST_TICK: u64 = 0;

pub fn tick() {
    let now = crate::arch::cpu::rdtsc();
    unsafe {
        if now.wrapping_sub(LAST_TICK) < 16_000_000 { return; }
        LAST_TICK = now;
    }

    let s = super::state();
    s.lock();
    let desktop_slot = s.windows.desktop;
    let focus = s.windows.focus;
    s.unlock();

    // 1) Pinta la ventana desktop (fondo + wallpaper).
    paint_desktop(desktop_slot);

    // 2) Pinta las ventanas de la Z-list, top-down, saltando el desktop.
    s.lock();
    s.windows.z_foreach_top_down(|slot| {
        if slot == desktop_slot { return; }
        if let Some(w) = s.windows.window(slot) {
            if !w.visible { return; }
            paint_window_frame(w, slot == focus);
        }
    });
    s.unlock();

    // 3) Pinta el cursor encima.
    paint_cursor();
}

fn paint_desktop(slot: u32) {
    if slot == WID_INVALID { return; }
    let s = super::state();
    s.lock();
    let (x, y, w, h) = match s.windows.window(slot) {
        Some(win) => (win.x as u32, win.y as u32, win.w as u32, win.h as u32),
        None => { s.unlock(); return; }
    };
    s.unlock();
    // Wallpaper procedural (mismo que welcome/desktop existentes).
    let fb = gop::get_backbuffer_fb();
    super::super::desktop::wallpaper::draw(&fb, crate::arch::cpu::rdtsc());
    let _ = (x, y, w, h);
}

fn paint_window_frame(w: &super::window::BmoWindow, focused: bool) {
    let x = w.x;
    let y = w.y;
    let ww = w.w;
    let wh = w.h;
    let fb = gop::get_backbuffer_fb();
    // Cuerpo de la ventana.
    fb.fill_rounded_rect(x as usize, y as usize, ww as usize, wh as usize, 14, theme::SURFACE_2);
    // Borde.
    let bd_color = if focused { theme::MINT } else { theme::SURFACE_BORDER };
    fb.draw_rect(x as usize, y as usize, ww as usize, wh as usize, bd_color, 1);
    // Title bar.
    let tb_color = if focused { 0xFF123045u32 } else { 0xFF1A2535u32 };
    fb.fill_rounded_rect(x as usize, y as usize, ww as usize, 36, 14, tb_color);
    // Sheen blanco.
    fb.fill_rect(x as usize + 1, y as usize + 1, ww as usize - 2, 1, theme::GLASS_HIGHLIGHT);
    // Línea inferior title bar.
    fb.fill_rect(x as usize + 1, y as usize + 35, ww as usize - 2, 1, 0xFF0A1A2Au32);
    // Traffic lights.
    fb.fill_circle((x + 18) as usize, (y + 18) as usize, 7, 0xFFFF5F56u32);
    fb.fill_circle((x + 38) as usize, (y + 18) as usize, 7, 0xFFFFBD2Eu32);
    fb.fill_circle((x + 58) as usize, (y + 18) as usize, 7, 0xFF27C93Fu32);
    // Título.
    if w.title_len > 0 {
        let t = &w.title[..w.title_len as usize];
        let tx = x + 80;
        let ty = y + 10;
        crate::desktop::render::draw_text(&fb, tx as u32, ty as u32, t, theme::TITLE);
    }
}

fn paint_cursor() {
    if !super::cursor::is_visible() { return; }
    let (x, y) = unsafe {
        let st = &crate::desktop::state::STATE;
        (st.mouse_x, st.mouse_y)
    };
    super::cursor::paint(x, y);
}
