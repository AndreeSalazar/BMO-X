//! Desktop input — thin wrapper over bmo_input crate.
//!
//! Legacy API preserved for compatibility. New code should use
//! `bmo_input::hal_ps2::Ps2Hal` directly.

#![allow(dead_code)]

use bmo_input::hal_ps2::Ps2Hal;
use bmo_input::hal::InputHal;
use core::sync::atomic::{AtomicBool, Ordering};

pub const SC_ESC: u8 = 0x01;

static INIT_DONE: AtomicBool = AtomicBool::new(false);

fn ensure_init() -> &'static mut Ps2Hal {
    static mut PS2: Option<Ps2Hal> = None;
    unsafe {
        if !INIT_DONE.load(Ordering::Relaxed) {
            PS2 = Some(Ps2Hal::new());
            PS2.as_mut().unwrap().init();
            INIT_DONE.store(true, Ordering::Relaxed);
        }
        PS2.as_mut().unwrap()
    }
}

pub fn poll_key() -> u8 {
    let ps2 = ensure_init();
    let mut buf = [bmo_input::event::InputEvent::empty(); 32];
    let n = ps2.poll(&mut buf);
    let mut last = None;
    for i in 0..n {
        if matches!(buf[i].kind, bmo_input::event::InputEventKind::KeyDown | bmo_input::event::InputEventKind::KeyUp) {
            last = Some(buf[i].code);
        }
    }
    last.unwrap_or(0)
}

pub fn poll_mouse() -> u64 {
    let ps2 = ensure_init();
    let mut buf = [bmo_input::event::InputEvent::empty(); 32];
    let n = ps2.poll(&mut buf);
    let mut x: i32 = 0; let mut y: i32 = 0; let mut btns: u64 = 0;
    for i in 0..n {
        match buf[i].kind {
            bmo_input::event::InputEventKind::MouseMove => {
                x = x.saturating_add(buf[i].mouse_dx() as i32);
                y = y.saturating_add(buf[i].mouse_dy() as i32);
            }
            bmo_input::event::InputEventKind::MouseButton => {
                btns = buf[i].mouse_buttons() as u64;
            }
            _ => {}
        }
    }
    let xi = (x.clamp(-32768, 32767) as i16) as u16 as u64;
    let yi = (y.clamp(-32768, 32767) as i16) as u16 as u64;
    xi | (yi << 16) | (btns << 32)
}
