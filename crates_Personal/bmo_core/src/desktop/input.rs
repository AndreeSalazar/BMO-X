//! Desktop input — PS/2/USB-legacy primary, native USB HID fallback.
//!
//! Prefer `bmo_input::hal_ps2::Ps2Hal` so BIOS USB Legacy Emulation keeps
//! USB keyboards/mice usable while the native xHCI/HID stack is incomplete.
//! Falls back to `bmo_uhid::UsbHidHal` only if PS/2 is unavailable.

use bmo_input::hal::InputHal;
use alloc::boxed::Box;
use crate::dev::console::{serial_write, serial_write_u64};

pub const SC_ESC: u8 = 0x01;

fn hal() -> &'static mut dyn InputHal {
    static mut ACTIVE: Option<Box<dyn InputHal>> = None;
    unsafe {
        if ACTIVE.is_none() {
            serial_write("[input] initializing...\n");
            let mut ps2 = Box::new(bmo_input::hal_ps2::Ps2Hal::new());
            let ok = ps2.init();
            serial_write("[input] ps2.init() = ");
            if ok {
                serial_write("OK\n");
                ACTIVE = Some(ps2);
            } else {
                serial_write("FAIL, fallback to native USB HID\n");
                let mut uhid = Box::new(bmo_uhid::UsbHidHal::new());
                let usb_ok = uhid.init();
                serial_write("[input] uhid.init() = ");
                if usb_ok {
                    serial_write("OK\n");
                    ACTIVE = Some(uhid);
                } else {
                    serial_write("FAIL, no input HAL ready\n");
                    ACTIVE = Some(ps2);
                }
            }
        }
        ACTIVE.as_mut().unwrap().as_mut()
    }
}

/// Poll raw scancode byte (bit 7 = released), compatible with welcome.rs process_scancode().
/// Returns 0 if no key event.
pub fn poll_raw_scancode() -> u8 {
    let h = hal();
    let mut buf = [bmo_input::event::InputEvent::empty(); 32];
    let n = h.poll(&mut buf);
    let mut last = 0u8;
    for i in 0..n {
        match buf[i].kind {
            bmo_input::event::InputEventKind::KeyDown => {
                last = buf[i].code;
            }
            bmo_input::event::InputEventKind::KeyUp => {
                last = buf[i].code | 0x80;
            }
            _ => {}
        }
    }
    if last != 0 {
        serial_write("[input] scancode=");
        serial_write_u64(last as u64, 2);
        serial_write("\n");
    }
    last
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
