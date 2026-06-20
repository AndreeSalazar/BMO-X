//! v2.0 — Input: PS/2 keyboard + mouse + USB HID → BMO events.
//!
//! En v2.0 los eventos se generan desde el poll principal en `wm::enter`
//! (no hay un thread dedicado todavía). La traducción de scancodes PS/2
//! a BMO_MSG_KEYDOWN/KEYUP/CHAR se hace aquí.

#![allow(dead_code)]

use super::super::v2::message::{BmoMsg, BmoMsgKind};
use super::super::v2::window::WID_INVALID;

/// Estado del teclado: shift, ctrl, alt, caps.
static mut KBD_LSHIFT: bool = false;
static mut KBD_RSHIFT: bool = false;
static mut KBD_CTRL:  bool = false;
static mut KBD_ALT:   bool = false;
static mut KBD_CAPS:  bool = false;

/// Estado del ratón (3 botones).
static mut MOUSE_L: bool = false;
static mut MOUSE_R: bool = false;
static mut MOUSE_M: bool = false;
static mut MOUSE_X: i32 = 0;
static mut MOUSE_Y: i32 = 0;
static mut ESC_LATCH: bool = false;

#[inline]
pub fn shift_held() -> bool { unsafe { KBD_LSHIFT || KBD_RSHIFT } }
#[inline]
pub fn caps_on() -> bool { unsafe { KBD_CAPS } }

/// Procesa un scancode PS/2 Set 1. Devuelve el (kind, wparam) a postear,
/// o None si fue un modifier puro.
pub fn translate_scancode(sc_raw: u8) -> Option<(BmoMsgKind, u8)> {
    let released = (sc_raw & 0x80) != 0;
    let sc = sc_raw & 0x7F;
    match sc {
        0x2A => { unsafe { KBD_LSHIFT = !released; } return None; }
        0x36 => { unsafe { KBD_RSHIFT = !released; } return None; }
        0x1D => { unsafe { KBD_CTRL  = !released; } return None; }
        0x38 => { unsafe { KBD_ALT   = !released; } return None; }
        0x3A => {
            if !released { unsafe { KBD_CAPS = !KBD_CAPS; } }
            return None;
        }
        0x01 => {
            // ESC
            if !released {
                unsafe { ESC_LATCH = true; }
                return Some((BmoMsgKind::KeyDown, 0x01));
            } else {
                return Some((BmoMsgKind::KeyUp, 0x01));
            }
        }
        _ => {}
    }
    if released { return None; }
    if sc == 0x1C { return Some((BmoMsgKind::KeyDown, 0x1C)); } // Enter
    if sc == 0x0E { return Some((BmoMsgKind::KeyDown, 0x08)); } // Backspace
    // Para el resto, devolvemos el scancode como virtual key genérico.
    Some((BmoMsgKind::KeyDown, sc))
}

/// Llamado por el WM en su loop principal. Hace poll de PS/2 y
/// traduce a mensajes BMO. Devuelve el número de eventos posteados.
pub fn poll_and_dispatch() -> u32 {
    let mut dispatched = 0u32;
    // Polling rápido: hasta 32 scancodes por tick.
    for _ in 0..32 {
        let sc = crate::desktop::poll_key();
        if sc == 0 { break; }
        if let Some((kind, vk)) = translate_scancode(sc) {
            post_to_focused(kind, vk as u64, 0);
            dispatched += 1;
        }
    }
    // Mouse (devuelve packed u64: x:i16 | y:i16 << 16 | buttons:u8 << 32).
    let packed = crate::desktop::poll_mouse();
    let mx = (packed & 0xFFFF) as i16 as i32;
    let my = ((packed >> 16) & 0xFFFF) as i16 as i32;
    let btns = ((packed >> 32) & 0xFF) as u8;
    unsafe {
        let old_x = MOUSE_X;
        let old_y = MOUSE_Y;
        MOUSE_X = mx;
        MOUSE_Y = my;
        let new_l = (btns & 1) != 0;
        let new_r = (btns & 2) != 0;
        let new_m = (btns & 4) != 0;
        if old_x != MOUSE_X || old_y != MOUSE_Y {
            let m = BmoMsg {
                kind: BmoMsgKind::MouseMove as u16,
                target: focused_wid(),
                source: 0, _pad0: 0,
                timestamp: 0, wparam: 0, lparam: 0,
                pt_x: MOUSE_X, pt_y: MOUSE_Y,
            };
            post_msg(m);
        }
        if new_l != MOUSE_L {
            MOUSE_L = new_l;
            let kind = if new_l { BmoMsgKind::LButtonDown } else { BmoMsgKind::LButtonUp };
            let m = BmoMsg {
                kind: kind as u16, target: focused_wid(),
                source: 0, _pad0: 0, timestamp: 0,
                wparam: 0, lparam: 0,
                pt_x: MOUSE_X, pt_y: MOUSE_Y,
            };
            post_msg(m);
        }
        if new_r != MOUSE_R {
            MOUSE_R = new_r;
            let kind = if new_r { BmoMsgKind::RButtonDown } else { BmoMsgKind::RButtonUp };
            let m = BmoMsg {
                kind: kind as u16, target: focused_wid(),
                source: 0, _pad0: 0, timestamp: 0,
                wparam: 0, lparam: 0,
                pt_x: MOUSE_X, pt_y: MOUSE_Y,
            };
            post_msg(m);
        }
        if new_m != MOUSE_M {
            MOUSE_M = new_m;
            let _ = new_m;
        }
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
    if let Some(slot) = qt.slot_for_tid(owner) {
        let msg = BmoMsg::new(kind, focus as u16, 0, wparam, lparam);
        let _ = super::event::post_coalesced(&mut qt.queues[slot as usize], msg);
    }
}

fn post_msg(m: BmoMsg) {
    let s = super::state();
    s.lock();
    let focus = s.windows.focus;
    let owner = s.windows.window(focus).map(|w| w.owner_tid).unwrap_or(0);
    s.unlock();
    if owner == 0 { return; }
    let qt = super::queue::queue_table();
    if let Some(slot) = qt.slot_for_tid(owner) {
        let _ = super::event::post_coalesced(&mut qt.queues[slot as usize], m);
    }
}

pub fn esc_pressed() -> bool {
    unsafe {
        let r = ESC_LATCH;
        ESC_LATCH = false;
        r
    }
}
