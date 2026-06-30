//! v1.0 — Taskbar: barra inferior del escritorio.
//!
//! Muestra ventanas minimizadas/restauradas, botones de título,
//! indicador de ventana activa.

#![allow(dead_code)]

use crate::bmo_core::desktop::theme;

const TASKBAR_HEIGHT: i32 = 48;
const BTN_W: i32 = 160;
const BTN_H: i32 = 36;
const BTN_GAP: i32 = 4;
const BTN_X0: i32 = 8;

pub fn taskbar_height() -> i32 { TASKBAR_HEIGHT }

pub fn taskbar_y(fb_height: i32) -> i32 { fb_height - TASKBAR_HEIGHT }

pub fn hit_test(px: i32, py: i32, fb_height: i32) -> Option<TaskbarHit> {
    let ty = taskbar_y(fb_height);
    if py < ty || py >= fb_height { return None; }
    if px < BTN_X0 { return Some(TaskbarHit::Background); }
    let idx = (px - BTN_X0) / (BTN_W + BTN_GAP);
    let bx = BTN_X0 + idx * (BTN_W + BTN_GAP);
    if px >= bx && px < bx + BTN_W && py >= ty + (TASKBAR_HEIGHT - BTN_H) / 2 && py < ty + (TASKBAR_HEIGHT - BTN_H) / 2 + BTN_H {
        Some(TaskbarHit::Button(idx as u32))
    } else {
        Some(TaskbarHit::Background)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TaskbarHit {
    Button(u32),
    Background,
}

pub fn paint(fb: &crate::bmo_core::ui::fb::Framebuffer, fb_height: i32) {
    let ty = taskbar_y(fb_height) as usize;
    let fb_width = fb.width as usize;

    fb.fill_rounded_rect(0, ty, fb_width, TASKBAR_HEIGHT as usize, 0, 0xFF0D1117);
    fb.fill_rect(0, ty, fb_width, 1, 0xFF30363D);

    let mut btn_x = BTN_X0;
    let btn_y = ty + ((TASKBAR_HEIGHT - BTN_H) / 2) as usize;

    let s = super::state();
    s.lock();
    let desktop = s.windows.desktop;

    s.windows.z_foreach_top_down(|slot| {
        if slot == desktop { return; }
        if let Some(w) = s.windows.window(slot) {
            if !w.used { return; }

            let is_minimized = w.minimized;
            let is_active = slot == s.windows.focus;
            let label_color = if is_active { theme::MINT } else { theme::TITLE };

            let bg = if is_active {
                0xFF1A2A35u32
            } else if is_minimized {
                0xFF161B22u32
            } else {
                0xFF0D1117u32
            };

            let bd = if is_active { theme::MINT } else { 0xFF30363Du32 };

            if btn_x + BTN_W > fb_width as i32 - 8 { return; }

            fb.fill_rounded_rect(btn_x as usize, btn_y, BTN_W as usize, BTN_H as usize, 8, bg);
            fb.draw_rect(btn_x as usize, btn_y, BTN_W as usize, BTN_H as usize, bd, 1);

            if is_active {
                fb.fill_rect(btn_x as usize, btn_y + BTN_H as usize - 3, BTN_W as usize, 3, theme::MINT);
            }

            let title_len = w.title_len as usize;
            if title_len > 0 {
                let mut short_title = [0u8; 18];
                let copy_len = title_len.min(16);
                short_title[..copy_len].copy_from_slice(&w.title[..copy_len]);
                if title_len > 16 {
                    short_title[14] = b'.';
                    short_title[15] = b'.';
                    short_title[16] = b'.';
                }
                let display_len = if title_len > 16 { 17 } else { copy_len };
                crate::bmo_core::desktop::render::draw_text(
                    fb,
                    (btn_x + 8) as u32,
                    (btn_y + (BTN_H as usize - 14) / 2) as u32,
                    &short_title[..display_len],
                    label_color,
                );
            }

            btn_x += BTN_W + BTN_GAP;
        }
    });
    s.unlock();
}

pub fn handle_click(btn_idx: u32, _fb_height: i32) -> Option<u32> {
    let s = super::state();
    s.lock();
    let desktop = s.windows.desktop;
    let mut visible_list: alloc::vec::Vec<u32> = alloc::vec::Vec::new();
    s.windows.z_foreach_top_down(|slot| {
        if slot == desktop { return; }
        if let Some(w) = s.windows.window(slot) {
            if w.used {
                visible_list.push(slot);
            }
        }
    });
    let target = visible_list.get(btn_idx as usize).copied();
    s.unlock();

    if let Some(slot) = target {
        let s2 = super::state();
        s2.lock();
        let was_minimized = s2.windows.window(slot).map(|w| w.minimized).unwrap_or(false);
        let was_focused = slot == s2.windows.focus;
        s2.unlock();

        if was_minimized {
            super::wm::restore_window(slot);
        } else if was_focused {
            super::wm::minimize_window(slot);
        } else {
            super::wm::bring_to_front(slot);
        }
        Some(slot)
    } else {
        None
    }
}

extern crate alloc;
