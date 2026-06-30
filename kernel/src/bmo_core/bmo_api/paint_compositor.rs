//! v3.0 — Compositor de pintado.
//!
//! Blit de surfaces al framebuffer, dirty-region tracking, selective repaint.

#![allow(dead_code)]

use super::window::WID_INVALID;
use crate::bmo_core::desktop::theme;
use core::sync::atomic::{AtomicU64, Ordering};

static LAST_TICK: AtomicU64 = AtomicU64::new(0);

pub fn tick() {
    let now = crate::cpu::rdtsc();
    if now.wrapping_sub(LAST_TICK.load(Ordering::Relaxed)) < 16_000_000 { return; }
    LAST_TICK.store(now, Ordering::Relaxed);

    let s = super::state();
    s.lock();
    let desktop_slot = s.windows.desktop;
    let focus = s.windows.focus;
    s.unlock();

    paint_desktop(desktop_slot);

    let slots_to_paint: [u32; 64] = {
        let s2 = super::state();
        s2.lock();
        let mut list = [WID_INVALID; 64];
        let mut idx = 0;
        s2.windows.z_foreach_top_down(|slot| {
            if slot == desktop_slot { return; }
            if idx < 64 {
                if let Some(w) = s2.windows.window(slot) {
                    if w.visible {
                        list[idx] = slot;
                        idx += 1;
                    }
                }
            }
        });
        s2.unlock();
        list
    };

    for slot in slots_to_paint {
        if slot == WID_INVALID { break; }
        let (is_dirty, has_rect, is_focused, surface_id, wx, wy, ww, wh, title_h, title_len, title_arr) = {
            let s3 = super::state();
            s3.lock();
            match s3.windows.window(slot) {
                Some(w) => {
                    let mut t = [0u8; 64];
                    let tl = w.title_len as usize;
                    t[..tl].copy_from_slice(&w.title[..tl]);
                    let r = (w.dirty, w.has_dirty_rect, slot == focus,
                            w.surface, w.x, w.y, w.w, w.h,
                            if w.style & super::window::style::WS_CAPTION != 0 { 36i32 } else { 0i32 },
                            tl, t);
                    s3.unlock();
                    r
                }
                None => { s3.unlock(); continue; }
            }
        };

        if is_dirty || has_rect {
            let fb = crate::bmo_core::ui::fb::backbuffer_fb();
            fb.fill_rounded_rect(wx as usize, wy as usize, ww as usize, wh as usize, 14, theme::SURFACE_2);
            let bd_color = if is_focused { theme::MINT } else { theme::SURFACE_BORDER };
            fb.draw_rect(wx as usize, wy as usize, ww as usize, wh as usize, bd_color, 1);
            let tb_color = if is_focused { 0xFF123045u32 } else { 0xFF1A2535u32 };
            fb.fill_rounded_rect(wx as usize, wy as usize, ww as usize, 36, 14, tb_color);
            fb.fill_rect(wx as usize + 1, wy as usize + 1, ww as usize - 2, 1, theme::GLASS_HIGHLIGHT);
            fb.fill_rect(wx as usize + 1, wy as usize + 35, ww as usize - 2, 1, 0xFF0A1A2Au32);
            fb.fill_circle((wx + 18) as usize, (wy + 18) as usize, 7, 0xFFFF5F56u32);
            fb.fill_circle((wx + 38) as usize, (wy + 18) as usize, 7, 0xFFFFBD2Eu32);
            fb.fill_circle((wx + 58) as usize, (wy + 18) as usize, 7, 0xFF27C93Fu32);
            if title_len > 0 {
                let t = &title_arr[..title_len];
                let tx = wx + 80;
                let ty = wy + 10;
                crate::bmo_core::desktop::render::draw_text(&fb, tx as u32, ty as u32, t, theme::TITLE);
            }
        }

        if surface_id != 0 && title_h > 0 {
            super::draw::blit_surface_to_fb(surface_id, wx, wy + title_h);
        }

        let s4 = super::state();
        s4.lock();
        if let Some(ww) = s4.windows.window_mut(slot) {
            ww.clear_dirty();
        }
        s4.unlock();
    }

    paint_taskbar();
    crate::dev::framebuffer::present();
    paint_cursor();
}

fn paint_desktop(slot: u32) {
    if slot == WID_INVALID { return; }
    let fb = crate::bmo_core::ui::fb::backbuffer_fb();
    super::super::desktop::wallpaper::draw(&fb, crate::cpu::rdtsc());
}

fn paint_cursor() {
    if !super::cursor::is_visible() { return; }
    let x = super::input::mouse_x();
    let y = super::input::mouse_y();
    super::cursor::paint(x, y);
}

fn paint_taskbar() {
    let fb = crate::bmo_core::ui::fb::backbuffer_fb();
    let (fbw, fbh) = unsafe { (crate::info::FB_WIDTH, crate::info::FB_HEIGHT) };
    let _ = fbw;
    super::taskbar::paint(&fb, fbh as i32);
}
