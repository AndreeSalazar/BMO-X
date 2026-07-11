//! v3.1 — Compositor Mac-like.
//!
//! Alpha-blended window rendering with soft shadows, frosted glass backdrop,
//! rounded corners, and animation ticks. Uses the fb.rs primitives:
//! - put_pixel_alpha (A_over_B blending)
//! - box_blur_3x3 (shadow softening)
//! - draw_rounded_rect (outline borders)
//! - blit_alpha (surface compositing)

#![allow(dead_code)]

use super::window::WID_INVALID;
use crate::desktop::theme;

use core::sync::atomic::{AtomicU64, Ordering};

static LAST_TICK: AtomicU64 = AtomicU64::new(0);
static LAST_ANIM_TICK: AtomicU64 = AtomicU64::new(0);

pub fn tick() {
    let now = crate::cpu::rdtsc();
    if now.wrapping_sub(LAST_TICK.load(Ordering::Relaxed)) < 16_000_000 { return; }
    LAST_TICK.store(now, Ordering::Relaxed);

    // Tick animations every ~16ms
    let anim_dt = now.wrapping_sub(LAST_ANIM_TICK.load(Ordering::Relaxed));
    LAST_ANIM_TICK.store(now, Ordering::Relaxed);
    let _dt_ms = if anim_dt > 0 { (anim_dt / 3_700_000) as u32 } else { 16 };

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
        let win_info = {
            let s3 = super::state();
            s3.lock();
            match s3.windows.window(slot) {
                Some(w) => {
                    let mut t = [0u8; 64];
                    let tl = w.title_len as usize;
                    t[..tl].copy_from_slice(&w.title[..tl]);
                    let r = (
                        w.dirty, w.has_dirty_rect, slot == focus,
                        w.surface, w.x, w.y, w.w, w.h,
                        if w.style & super::window::style::WS_CAPTION != 0 { 36i32 } else { 0i32 },
                        tl, t,
                    );
                    s3.unlock();
                    r
                }
                None => { s3.unlock(); continue; }
            }
        };
        let (is_dirty, has_rect, is_focused, surface_id, wx, wy, ww, wh, title_h, title_len, title_arr) = win_info;
        let rounded_radius = 14usize;

        if is_dirty || has_rect {
            let fb = crate::ui::fb::backbuffer_fb();

            // 1. Drop shadow (offset + blur for depth)
            paint_window_shadow(&fb, wx, wy, ww, wh, rounded_radius);

            // 2. Frosted glass backdrop (blur the area behind the window first, then overlay)
            paint_glass_backdrop(&fb, wx, wy, ww, wh, rounded_radius);

            // 3. Window body (semi-transparent, dark)
            fb.fill_rect_alpha(wx as usize, wy as usize, ww as usize, wh as usize, theme::SURFACE_2);
            fb.fill_rounded_rect(wx as usize, wy as usize, ww as usize, wh as usize, rounded_radius, theme::SURFACE_2);

            // 4. Rounded border with inner glow
            let bd_color = if is_focused { theme::MINT } else { theme::SURFACE_BORDER };
            fb.draw_rounded_rect(wx as usize, wy as usize, ww as usize, wh as usize, rounded_radius, bd_color, 1);
            if is_focused {
                // Outer neon glow ring
                fb.draw_rounded_rect(
                    (wx - 1) as usize, (wy - 1) as usize,
                    (ww + 2) as usize, (wh + 2) as usize,
                    rounded_radius + 1, 0x33FFFFFF, 1,
                );
            }

            // 5. Title bar with glass effect
            let tb_color = if is_focused { 0xBB182A40u32 } else { 0xBB1A2535u32 };
            fb.fill_rect_alpha(wx as usize, wy as usize, ww as usize, 36, tb_color);
            // Top highlight strip (glass reflection)
            fb.fill_rect_alpha(wx as usize + 1, wy as usize + 1, (ww - 2) as usize, 1, theme::GLASS_HIGHLIGHT);
            // Bottom separator
            fb.fill_rect_alpha(wx as usize + 1, wy as usize + 35, (ww - 2) as usize, 1, 0x330A1A2A);

            // 6. Traffic lights (close/min/max)
            fb.fill_circle((wx + 18) as usize, (wy + 18) as usize, 7, 0xFFFF5F56u32);  // red
            fb.fill_circle((wx + 38) as usize, (wy + 18) as usize, 7, 0xFFFFBD2Eu32);  // yellow
            fb.fill_circle((wx + 58) as usize, (wy + 18) as usize, 7, 0xFF27C93Fu32);  // green

            // 7. Window title
            if title_len > 0 {
                let t = &title_arr[..title_len];
                let tx = wx + 80;
                let ty = wy + 10;
                crate::desktop::render::draw_text(&fb, tx as u32, ty as u32, t, theme::TITLE);
            }

            // 8. Blit surface content (only when window is dirty)
            if surface_id != 0 && title_h > 0 {
                super::draw::blit_surface_alpha_to_fb(surface_id, wx, wy + title_h);
            }
        }

        // Clear dirty flags
        let s4 = super::state();
        s4.lock();
        if let Some(ww) = s4.windows.window_mut(slot) {
            ww.clear_dirty();
        }
        s4.unlock();
    }

    paint_taskbar();
    paint_menu_bar();
    paint_cursor();
    crate::dev::framebuffer::present();
}

/// Global menu bar at the top of the screen (Mac-style).
fn paint_menu_bar() {
    unsafe {
        if !MENUBAR_DIRTY { return; }
    }
    let fb = crate::ui::fb::backbuffer_fb();
    let (fbw, _fbh) = unsafe { (crate::info::FB_WIDTH, crate::info::FB_HEIGHT) };
    let mh = theme::MENU_HEIGHT as usize;

    // Blur backdrop
    fb.box_blur_3x3(0, 0, fbw as usize, mh);
    fb.fill_rect_alpha(0, 0, fbw as usize, mh, theme::MENU_BG);
    // Bottom separator
    fb.fill_rect_alpha(0, mh, fbw as usize, 1, 0x5530363D);

    // Menu items
    let menus: &[&[u8]] = &[b"File", b"Edit", b"View", b"Window", b"Help"];
    let mut cx = 16usize;
    for &menu in menus {
        crate::desktop::render::draw_text(
            &fb, cx as u32, 7, menu, theme::MENU_TEXT,
        );
        cx += menu.len() * 8 + 24;
    }

    // Right side: clock area (simplified)
    let clock_label = b"BMO v2.0";
    let cw = clock_label.len() * 8;
    crate::desktop::render::draw_text(
        &fb, (fbw as usize - cw - 16) as u32, 7, clock_label, theme::SUBTITLE,
    );
    unsafe { MENUBAR_DIRTY = false; }
}

/// Soft drop shadow behind a window.
fn paint_window_shadow(fb: &crate::ui::fb::Framebuffer, x: i32, y: i32, w: i32, h: i32, _r: usize) {
    let sx = (x + 6) as usize;
    let sy = (y + 10) as usize;
    let sw = w as usize;
    let sh = h as usize;
    // Draw dark offset rectangle
    fb.fill_rect_alpha(sx, sy, sw, sh, 0x40000000);
    // Soften with blur
    let blur_x = sx.saturating_sub(4);
    let blur_y = sy.saturating_sub(4);
    let blur_w = (sw + 8).min(fb.width.saturating_sub(blur_x));
    let blur_h = (sh + 8).min(fb.height.saturating_sub(blur_y));
    if blur_w > 8 && blur_h > 8 {
        fb.box_blur_3x3(blur_x, blur_y, blur_w, blur_h);
    }
}

/// Frosted glass effect: blur the area behind the window, then overlay tint.
fn paint_glass_backdrop(fb: &crate::ui::fb::Framebuffer, x: i32, y: i32, w: i32, h: i32, _r: usize) {
    let gx = x as usize;
    let gy = y as usize;
    let gw = w as usize;
    let gh = h as usize;
    // Blur the background area where the window will be drawn
    fb.box_blur_3x3(gx, gy, gw.min(fb.width.saturating_sub(gx)), gh.min(fb.height.saturating_sub(gy)));
    // Tint with dark overlay for glass effect
    fb.fill_rect_alpha(gx, gy, gw.min(fb.width.saturating_sub(gx)), gh.min(fb.height.saturating_sub(gy)), 0x1A0A1018);
}

/// Desktop wallpaper needs redraw. Set on first frame, resolution change, wallpaper change.
static mut DESKTOP_DIRTY: bool = true;

/// Mark desktop for redraw (e.g., wallpaper change, resolution change).
pub fn invalidate_desktop() {
    unsafe { DESKTOP_DIRTY = true; }
}

fn paint_desktop(slot: u32) {
    if slot == WID_INVALID { return; }
    unsafe {
        if DESKTOP_DIRTY {
            let fb = crate::ui::fb::backbuffer_fb();
            crate::desktop::wallpaper::draw(&fb, crate::cpu::rdtsc());
            DESKTOP_DIRTY = false;
        }
    }
}

fn paint_cursor() {
    if !super::cursor::is_visible() { return; }
    let x = super::input::mouse_x();
    let y = super::input::mouse_y();
    super::cursor::paint(x, y);
}

static mut TASKBAR_DIRTY: bool = true;
static mut MENUBAR_DIRTY: bool = true;

fn paint_taskbar() {
    unsafe {
        if !TASKBAR_DIRTY { return; }
    }
    let fb = crate::ui::fb::backbuffer_fb();
    let (fbw, fbh) = unsafe { (crate::info::FB_WIDTH, crate::info::FB_HEIGHT) };
    let _ = fbw;
    super::taskbar::paint(&fb, fbh as i32);
    unsafe { TASKBAR_DIRTY = false; }
}
