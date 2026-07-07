//! PS/2 Input — keyboard and mouse polling for the Ring 0 desktop.
//!
//! Uses unified `crate::port_io` for I/O. Tracks cursor position and
//! dispatches scancodes to `bmo_api` for translation.

#![allow(dead_code)]

use crate::port_io;

pub const SC_ESC: u8 = 0x01;
const SC_F8: u8 = 0x42;
const SC_F9: u8 = 0x43;
const SC_F10: u8 = 0x44;

static mut CTRL_HELD: bool = false;
static mut ALT_HELD: bool = false;
static mut HOTKEY_TOGGLED: bool = false;

pub fn poll_key() -> u8 {
    let status = unsafe { port_io::inb(0x64) };
    if status == 0xFF { return 0; }
    if (status & 0x01) == 0 { return 0; }
    if (status & 0x20) != 0 {
        let b = unsafe { port_io::inb(0x60) };
        process_mouse_byte(b);
        return 0;
    }
    let sc = unsafe { port_io::inb(0x60) };

    match sc {
        SC_F9 => {
            let on = crate::cabina::is_overlay_enabled();
            crate::cabina::set_overlay_enabled(!on);
            crate::cabina::cycle_tab();
            super::sound::beep(660, 30);
            super::state::mark_dirty();
        }
        SC_F10 => {
            crate::cabina::cycle_tab();
            super::sound::beep(550, 20);
            super::state::mark_dirty();
        }
        SC_F8 => {
            crate::cabina::cycle_query();
            super::sound::beep(770, 20);
            super::state::mark_dirty();
        }
        0x1D => { unsafe { CTRL_HELD = true; } }
        0x9D => { unsafe { CTRL_HELD = false; HOTKEY_TOGGLED = false; } }
        0x38 => { unsafe { ALT_HELD = true; } }
        0xB8 => { unsafe { ALT_HELD = false; HOTKEY_TOGGLED = false; } }
        _ => {}
    }
    unsafe {
        if CTRL_HELD && ALT_HELD && !HOTKEY_TOGGLED {
            HOTKEY_TOGGLED = true;
            let on = crate::cabina::is_overlay_enabled();
            crate::cabina::set_overlay_enabled(!on);
            super::sound::beep(660, 30);
            super::state::mark_dirty();
        }
    }
    sc
}

// ── Mouse ──────────────────────────────────────────────────────────

static mut MOUSE_X: i32 = 960;
static mut MOUSE_Y: i32 = 540;
static mut MOUSE_BUTTONS: u8 = 0;
static mut MOUSE_PKT: [u8; 3] = [0; 3];
static mut MOUSE_PKT_IDX: usize = 0;

fn process_mouse_byte(b: u8) {
    unsafe {
        MOUSE_PKT[MOUSE_PKT_IDX] = b;
        MOUSE_PKT_IDX += 1;
        if MOUSE_PKT_IDX < 3 { return; }
        MOUSE_PKT_IDX = 0;

        let b0 = MOUSE_PKT[0];
        if (b0 & 0x08) == 0 { return; }
        if (b0 & 0xC0) != 0 { return; }

        let dx_raw = MOUSE_PKT[1] as i32;
        let dy_raw = MOUSE_PKT[2] as i32;
        let dx = if (b0 & 0x10) != 0 { dx_raw - 0x100 } else { dx_raw };
        let dy = if (b0 & 0x20) != 0 { dy_raw - 0x100 } else { dy_raw };

        MOUSE_X = (MOUSE_X + dx).clamp(0, crate::info::FB_WIDTH as i32 - 1);
        MOUSE_Y = (MOUSE_Y - dy).clamp(0, crate::info::FB_HEIGHT as i32 - 1);
        MOUSE_BUTTONS = b0 & 0x07;
    }
}

/// Returns `(x:i16) | (y:i16 << 16) | (buttons:u8 << 32)`.
pub fn poll_mouse() -> u64 {
    unsafe {
        let mut limit = 0;
        loop {
            let status = port_io::inb(0x64);
            if status == 0xFF { break; }
            if (status & 0x21) != 0x21 { break; }
            let b = port_io::inb(0x60);
            process_mouse_byte(b);
            limit += 1;
            if limit > 64 { break; }
        }
        let x = (MOUSE_X as i16) as u16 as u64;
        let y = (MOUSE_Y as i16) as u16 as u64;
        let bt = MOUSE_BUTTONS as u64;
        x | (y << 16) | (bt << 32)
    }
}
