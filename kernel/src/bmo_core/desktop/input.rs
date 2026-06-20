//! PS/2 Input — keyboard and mouse polling for the Ring 0 desktop.
//!
//! Keyboard: PS/2 Set 1 scancode polling, modifier tracking (Ctrl, Alt),
//! F9 and Ctrl+Alt hotkey for the diag overlay toggle.
//!
//! Mouse: PS/2 packet accumulator, relative movement and button state.

#![allow(dead_code)]

use crate::boot_info;

pub const SC_ESC: u8 = 0x01;
const SC_F9: u8 = 0x43;

static mut CTRL_HELD: bool = false;
static mut ALT_HELD: bool = false;
static mut HOTKEY_TOGGLED: bool = false;

pub fn poll_key() -> u8 {
    let status: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16); }
    if status == 0xFF { return 0; }
    if (status & 0x01) == 0 { return 0; }
    if (status & 0x20) != 0 {
        let b: u8;
        unsafe {
            core::arch::asm!("in al, dx", out("al") b, in("dx") 0x60u16);
            process_mouse_byte(b);
        }
        return 0;
    }
    let sc: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") sc, in("dx") 0x60u16); }

    unsafe {
        match sc {
            SC_F9 => {
                let on = crate::bmo_core::diag::is_overlay_enabled();
                crate::bmo_core::diag::set_overlay_enabled(!on);
                super::sound::beep(660, 30);
                super::state::mark_dirty();
            }
            0x1D => { CTRL_HELD = true; }
            0x9D => { CTRL_HELD = false; HOTKEY_TOGGLED = false; }
            0x38 => { ALT_HELD = true; }
            0xB8 => { ALT_HELD = false; HOTKEY_TOGGLED = false; }
            _ => {}
        }
        if CTRL_HELD && ALT_HELD && !HOTKEY_TOGGLED {
            HOTKEY_TOGGLED = true;
            let on = crate::bmo_core::diag::is_overlay_enabled();
            crate::bmo_core::diag::set_overlay_enabled(!on);
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
static mut MOUSE_INIT_DONE: bool = false;

#[inline(always)]
unsafe fn ps2_wait_input() {
    for _ in 0..1_000 {
        let s: u8;
        core::arch::asm!("in al, dx", out("al") s, in("dx") 0x64u16);
        if s == 0xFF { return; }
        if (s & 0x02) == 0 { return; }
    }
}

#[inline(always)]
unsafe fn ps2_wait_output() {
    for _ in 0..1_000 {
        let s: u8;
        core::arch::asm!("in al, dx", out("al") s, in("dx") 0x64u16);
        if s == 0xFF { return; }
        if (s & 0x01) != 0 { return; }
    }
}

fn mouse_init() {
    unsafe {
        if MOUSE_INIT_DONE { return; }
        MOUSE_INIT_DONE = true;
        crate::device::serial::serial_write("[desktop] Bypassing legacy PS/2 mouse setup for pure UEFI.\n");
    }
}

unsafe fn process_mouse_byte(b: u8) {
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

    MOUSE_X = (MOUSE_X + dx).clamp(0, boot_info::FB_WIDTH as i32 - 1);
    MOUSE_Y = (MOUSE_Y - dy).clamp(0, boot_info::FB_HEIGHT as i32 - 1);
    MOUSE_BUTTONS = b0 & 0x07;
}

/// Returns `(x:i16) | (y:i16 << 16) | (buttons:u8 << 32)`.
pub fn poll_mouse() -> u64 {
    mouse_init();
    unsafe {
        let mut limit = 0;
        loop {
            let status: u8;
            core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16);
            if status == 0xFF { break; }
            if (status & 0x21) != 0x21 { break; }
            let b: u8;
            core::arch::asm!("in al, dx", out("al") b, in("dx") 0x60u16);
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
