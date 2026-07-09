//! v3.0 — Input: PS/2 keyboard + mouse → BMO events.
//!
//! Alt-Tab cycling, resize edge detection, complete VK mapping.

#![allow(dead_code)]

use super::message::{BmoMsg, BmoMsgKind};
use super::window::WID_INVALID;
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static KBD_LSHIFT: AtomicBool = AtomicBool::new(false);
static KBD_RSHIFT: AtomicBool = AtomicBool::new(false);
static KBD_CTRL:  AtomicBool = AtomicBool::new(false);
static KBD_ALT:   AtomicBool = AtomicBool::new(false);
static KBD_CAPS:  AtomicBool = AtomicBool::new(false);

static MOUSE_L: AtomicBool = AtomicBool::new(false);
static MOUSE_R: AtomicBool = AtomicBool::new(false);
static MOUSE_M: AtomicBool = AtomicBool::new(false);
static MOUSE_X: AtomicI32 = AtomicI32::new(0);
static MOUSE_Y: AtomicI32 = AtomicI32::new(0);
static ESC_LATCH: AtomicBool = AtomicBool::new(false);

#[inline]
pub fn shift_held() -> bool { KBD_LSHIFT.load(Ordering::Relaxed) || KBD_RSHIFT.load(Ordering::Relaxed) }
#[inline]
pub fn caps_on() -> bool { KBD_CAPS.load(Ordering::Relaxed) }
#[inline]
pub fn alt_held() -> bool { KBD_ALT.load(Ordering::Relaxed) }

pub fn mouse_x() -> i32 { MOUSE_X.load(Ordering::Relaxed) }
pub fn mouse_y() -> i32 { MOUSE_Y.load(Ordering::Relaxed) }

pub fn translate_scancode(sc_raw: u8) -> Option<(BmoMsgKind, u8)> {
    let released = (sc_raw & 0x80) != 0;
    let sc = sc_raw & 0x7F;
    match sc {
        0x2A => { KBD_LSHIFT.store(!released, Ordering::Relaxed); return None; }
        0x36 => { KBD_RSHIFT.store(!released, Ordering::Relaxed); return None; }
        0x1D => { KBD_CTRL.store(!released, Ordering::Relaxed); return None; }
        0x38 => { KBD_ALT.store(!released, Ordering::Relaxed); return None; }
        0x3A => {
            if !released { KBD_CAPS.store(!KBD_CAPS.load(Ordering::Relaxed), Ordering::Relaxed); }
            return None;
        }
        _ => {}
    }
    let kind = if released { BmoMsgKind::KeyUp } else { BmoMsgKind::KeyDown };
    let vk = match sc {
        0x01 => { ESC_LATCH.store(true, Ordering::Relaxed); 0x1B }, // ESC
        0x0E => 0x08, // Backspace
        0x0F => 0x09, // Tab
        0x1C => 0x0D, // Enter
        0x39 => 0x20, // Space
        0x4B => 0x25, // Left
        0x48 => 0x26, // Up
        0x4D => 0x27, // Right
        0x50 => 0x28, // Down
        0x47 => 0x24, // Home
        0x4F => 0x23, // End
        0x49 => 0x21, // PageUp
        0x51 => 0x22, // PageDown
        0x52 => 0x2D, // Insert
        0x53 => 0x2E, // Delete
        0x3B => 0x70, // F1
        0x3C => 0x71, // F2
        0x3D => 0x72, // F3
        0x3E => 0x73, // F4
        0x3F => 0x74, // F5
        0x40 => 0x75, // F6
        0x41 => 0x76, // F7
        0x42 => 0x77, // F8
        0x43 => 0x78, // F9
        0x44 => 0x79, // F10
        0x57 => 0x7A, // F11
        0x58 => 0x7B, // F12
        0x02 => b'1', 0x03 => b'2', 0x04 => b'3', 0x05 => b'4',
        0x06 => b'5', 0x07 => b'6', 0x08 => b'7', 0x09 => b'8',
        0x0A => b'9', 0x0B => b'0',
        0x1E => b'A', 0x30 => b'B', 0x2E => b'C', 0x20 => b'D',
        0x12 => b'E', 0x21 => b'F', 0x22 => b'G', 0x23 => b'H',
        0x17 => b'I', 0x24 => b'J', 0x25 => b'K', 0x26 => b'L',
        0x32 => b'M', 0x31 => b'N', 0x18 => b'O', 0x19 => b'P',
        0x10 => b'Q', 0x13 => b'R', 0x1F => b'S', 0x14 => b'T',
        0x16 => b'U', 0x2F => b'V', 0x11 => b'W', 0x2D => b'X',
        0x15 => b'Y', 0x2C => b'Z',
        0x1A => b'[', 0x1B => b']', 0x2B => b'\\', 0x27 => b';',
        0x28 => b'\'', 0x33 => b',', 0x34 => b'.', 0x35 => b'/',
        0x29 => b'`', 0x0C => b'-', 0x0D => b'=',
        _ => sc,
    };
    Some((kind, vk))
}

pub fn poll_and_dispatch() -> u32 {
    let mut dispatched = 0u32;
    for _ in 0..32 {
        let sc = crate::desktop::poll_key();
        if sc == 0 { break; }
        if let Some((kind, vk)) = translate_scancode(sc) {
            if vk == 0x09 && kind == BmoMsgKind::KeyDown && alt_held() {
                super::wm::alt_tab();
                dispatched += 1;
                continue;
            }
            post_to_focused(kind, vk as u64, 0);
            dispatched += 1;
        }
    }
    let packed = crate::desktop::poll_mouse();
    let mx = (packed & 0xFFFF) as i16 as i32;
    let my = ((packed >> 16) & 0xFFFF) as i16 as i32;
    let btns = ((packed >> 32) & 0xFF) as u8;
    let old_x = MOUSE_X.load(Ordering::Relaxed);
    let old_y = MOUSE_Y.load(Ordering::Relaxed);
    MOUSE_X.store(mx, Ordering::Relaxed);
    MOUSE_Y.store(my, Ordering::Relaxed);

    let l_was_down = MOUSE_L.load(Ordering::Relaxed);
    let new_l = (btns & 1) != 0;
    let new_r = (btns & 2) != 0;
    let new_m = (btns & 4) != 0;

    if super::wm::is_resizing() {
        if !new_l {
            super::wm::end_resize();
        } else {
            super::wm::update_resize(mx, my);
        }
        return dispatched;
    }

    if super::wm::is_dragging() {
        if !new_l {
            super::wm::end_drag();
        } else {
            super::wm::update_drag(mx, my);
        }
        return dispatched;
    }

    if old_x != mx || old_y != my {
        let target = {
            let s = super::state();
            s.lock();
            let t = s.windows.focus;
            s.unlock();
            t
        };
        let m = BmoMsg {
            kind: BmoMsgKind::MouseMove as u16,
            target: if target == WID_INVALID { 0 } else { target as u16 },
            source: 0, _pad0: 0,
            timestamp: 0, wparam: 0, lparam: 0,
            pt_x: mx, pt_y: my,
        };
        post_msg(m);
    }

    if new_l != l_was_down {
        MOUSE_L.store(new_l, Ordering::Relaxed);
        if new_l {
            let (fbw, fbh) = unsafe { (crate::info::FB_WIDTH as i32, crate::info::FB_HEIGHT as i32) };
            let _ = fbw;
            if my >= fbh - super::taskbar::taskbar_height() {
                if let Some(super::taskbar::TaskbarHit::Button(idx)) = super::taskbar::hit_test(mx, my, fbh) {
                    super::taskbar::handle_click(idx, fbh);
                }
            } else if super::wm::is_dragging() || super::wm::is_resizing() {
                // already in progress
            } else {
                let hit = super::wm::hit_test(mx, my);
                if hit != WID_INVALID {
                    let tb = super::wm::title_btn_hit_test(hit, mx, my);
                    if tb != super::wm::TitleBtn::None {
                        super::wm::handle_title_btn_click(hit, tb);
                    } else {
                        let edge = super::wm::edge_hit_test(mx, my);
                        if edge != 0 {
                            super::wm::start_resize(hit, edge, mx, my);
                        } else {
                            super::wm::raise_and_focus(hit);
                            super::wm::start_drag(hit, mx, my);
                        }
                    }
                }
            }
        } else {
            let kind = BmoMsgKind::LButtonUp;
            let m = BmoMsg {
                kind: kind as u16, target: focused_wid(),
                source: 0, _pad0: 0, timestamp: 0,
                wparam: 0, lparam: 0,
                pt_x: mx, pt_y: my,
            };
            post_msg(m);
        }
    }

    if new_r != MOUSE_R.load(Ordering::Relaxed) {
        MOUSE_R.store(new_r, Ordering::Relaxed);
        let kind = if new_r { BmoMsgKind::RButtonDown } else { BmoMsgKind::RButtonUp };
        let m = BmoMsg {
            kind: kind as u16, target: focused_wid(),
            source: 0, _pad0: 0, timestamp: 0,
            wparam: 0, lparam: 0,
            pt_x: mx, pt_y: my,
        };
        post_msg(m);
    }

    if new_m != MOUSE_M.load(Ordering::Relaxed) {
        MOUSE_M.store(new_m, Ordering::Relaxed);
        let kind = if new_m { BmoMsgKind::MButtonDown } else { BmoMsgKind::MButtonUp };
        let m = BmoMsg {
            kind: kind as u16, target: focused_wid(),
            source: 0, _pad0: 0, timestamp: 0,
            wparam: 0, lparam: 0,
            pt_x: mx, pt_y: my,
        };
        post_msg(m);
    }
    dispatched
}

fn focused_wid() -> u16 {
    let s = super::state();
    s.lock();
    let w = s.windows.focus;
    s.unlock();
    if w == WID_INVALID { 0 } else { w as u16 }
}

fn post_to_focused(kind: BmoMsgKind, wparam: u64, lparam: u64) {
    let s = super::state();
    s.lock();
    let focus = s.windows.focus;
    let owner = s.windows.window(focus).map(|w| w.owner_tid).unwrap_or(0);
    s.unlock();
    if owner == 0 { return; }
    let qt = super::queue::queue_table();
    qt.acquire();
    if let Some(slot) = qt.slot_for_tid(owner) {
        let msg = BmoMsg::new(kind, focus as u16, 0, wparam, lparam);
        let _ = super::event::post_coalesced(&mut qt.queues[slot as usize], msg);
    }
    qt.release();
}

fn post_msg(m: BmoMsg) {
    let s = super::state();
    s.lock();
    let focus = s.windows.focus;
    let owner = s.windows.window(focus).map(|w| w.owner_tid).unwrap_or(0);
    s.unlock();
    if owner == 0 { return; }
    let qt = super::queue::queue_table();
    qt.acquire();
    if let Some(slot) = qt.slot_for_tid(owner) {
        let _ = super::event::post_coalesced(&mut qt.queues[slot as usize], m);
    }
    qt.release();
}

pub fn esc_pressed() -> bool {
    let r = ESC_LATCH.load(Ordering::Relaxed);
    ESC_LATCH.store(false, Ordering::Relaxed);
    r
}

// ── Spanish keyboard layout (CP437 compatible) ──────────────────

/// Translate a Set 1 scancode to a character using Spanish layout.
/// Returns None for non-character keys (arrows, modifiers, F-keys).
pub fn scancode_to_char_es(sc: u8) -> Option<u8> {
    let released = (sc & 0x80) != 0;
    if released { return None; }
    let sc = sc & 0x7F;
    let caps = caps_on();
    let shift = shift_held();
    let upper = caps ^ shift;
    match sc {
        0x02..=0x09 => Some(if upper { b'1' - 1 + sc as u8 - 1 } else { b'1' - 1 + sc as u8 - 1 }),
        0x0A => { None /* accent key — needs compose */ },
        0x0B => Some(b'0'),
        0x0C => Some(if upper { b'?' } else { b'\'' }),
        0x0D => Some(if upper { 0xA8 } else { 0xAD }),          // ¿ / ¡
        0x10 | 0x1E | 0x1F | 0x20 | 0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26
        | 0x11 | 0x12 | 0x13 | 0x14 | 0x15 | 0x16 | 0x17 | 0x18 | 0x19
        | 0x2C | 0x2D | 0x2E | 0x2F | 0x30 | 0x31 | 0x32 => {
            let base: u8 = if upper { b'A' - 0x1E } else { b'a' - 0x1E };
            Some(base + sc as u8)
        },
        0x27 => Some(if upper { 0xA5 } else { 0xA4 }),          // Ñ / ñ
        0x33 => Some(if upper { b';' } else { b',' }),
        0x34 => Some(if upper { b':' } else { b'.' }),
        0x35 => Some(if upper { b'_' } else { b'-' }),
        0x39 => Some(b' '),
        0x0E => Some(0x08),                                      // Backspace
        0x1C => Some(b'\n'),                                     // Enter
        0x53 => Some(0x7F),                                      // Delete
        _ => None,
    }
}
