//! Desktop input — USB HID primary, PS/2 fallback.
//!
//! Uses `bmo_uhid::UsbHidHal` (implements InputHal) when xHCI is available.
//! Falls back to `bmo_input::hal_ps2::Ps2Hal` otherwise.

#![allow(dead_code)]

use bmo_input::hal::InputHal;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};

pub const SC_ESC: u8 = 0x01;
static INIT_DONE: AtomicBool = AtomicBool::new(false);

fn hal() -> &'static mut dyn InputHal {
    static mut ACTIVE: Option<Box<dyn InputHal>> = None;
    unsafe {
        if ACTIVE.is_none() {
            // Try USB HID first
            let mut uhid = Box::new(bmo_uhid::UsbHidHal::new());
            if uhid.init() {
                ACTIVE = Some(uhid);
            } else {
                // Fallback to PS/2
                let mut ps2 = Box::new(bmo_input::hal_ps2::Ps2Hal::new());
                ps2.init();
                ACTIVE = Some(ps2);
            }
        }
        ACTIVE.as_mut().unwrap().as_mut()
    }
}

pub fn poll_key() -> u8 {
    let h = hal();
    let mut buf = [bmo_input::event::InputEvent::empty(); 32];
    let n = h.poll(&mut buf);
    let mut last = None;
    for i in 0..n {
        if matches!(buf[i].kind, bmo_input::event::InputEventKind::KeyDown | bmo_input::event::InputEventKind::KeyUp) {
            last = Some(buf[i].code);
        }
    }
    last.unwrap_or(0)
}

pub fn poll_mouse() -> u64 {
    let h = hal();
    let mut buf = [bmo_input::event::InputEvent::empty(); 32];
    let n = h.poll(&mut buf);
    let mut x: i32 = 0; let mut y: i32 = 0; let mut btns: u64 = 0;
    for i in 0..n {
        match buf[i].kind {
            bmo_input::event::InputEventKind::MouseMove => {
                x = x.saturating_add(buf[i].mouse_dx() as i32);
                y = y.saturating_add(buf[i].mouse_dy() as i32);
            }
            bmo_input::event::InputEventKind::MouseButton => { btns = buf[i].mouse_buttons() as u64; }
            _ => {}
        }
    }
    let xi = (x.clamp(-32768, 32767) as i16) as u16 as u64;
    let yi = (y.clamp(-32768, 32767) as i16) as u16 as u64;
    xi | (yi << 16) | (btns << 32)
}
