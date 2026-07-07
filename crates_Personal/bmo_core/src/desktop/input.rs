//! PS/2 Input — keyboard and mouse polling with proper init sequences.

#![allow(dead_code)]

use crate::port_io;

pub const SC_ESC: u8 = 0x01;
const SC_F8: u8 = 0x42;
const SC_F9: u8 = 0x43;
const SC_F10: u8 = 0x44;

static mut CTRL_HELD: bool = false;
static mut ALT_HELD: bool = false;
static mut SHIFT_HELD: bool = false;
static mut HOTKEY_TOGGLED: bool = false;

// ── PS/2 Initialization ─────────────────────────────────────────

/// Send a command byte to the PS/2 controller (port 0x64).
unsafe fn ps2_send_cmd(cmd: u8) -> bool {
    if !port_io::ps2_wait_input() { return false; }
    port_io::outb(0x64, cmd);
    true
}

/// Send a data byte to the PS/2 data port (port 0x60).
unsafe fn ps2_send_data(data: u8) -> bool {
    if !port_io::ps2_wait_input() { return false; }
    port_io::outb(0x60, data);
    true
}

/// Read a byte from the PS/2 data port (port 0x60) with timeout.
unsafe fn ps2_read_data() -> Option<u8> {
    for _ in 0..1000 {
        let status = port_io::inb(0x64);
        if status == 0xFF { return None; }
        if (status & 0x01) != 0 {
            return Some(port_io::inb(0x60));
        }
    }
    None
}

/// Initialize PS/2 keyboard: enable scanning, set LEDs.
/// Returns early if no PS/2 controller present (port returns 0xFF).
pub fn keyboard_init() {
    unsafe {
        if KEYBOARD_INIT_DONE { return; }

        // Check if PS/2 port is alive
        let status = port_io::inb(0x64);
        if status == 0xFF {
            crate::dev::console::serial_write("[input] WARN: no PS/2 controller (port=0xFF), keyboard init skipped\n");
            KEYBOARD_INIT_DONE = true;
            return;
        }

        // Drain stale data
        while let Some(_) = ps2_read_data() {}

        // Enable scanning: disable then re-enable
        if !ps2_send_data(0xF5) { KEYBOARD_INIT_DONE = true; return; }
        ps2_read_data();
        if !ps2_send_data(0xF4) { KEYBOARD_INIT_DONE = true; return; }
        ps2_read_data();

        // Turn on Num Lock LED
        if !ps2_send_data(0xED) { KEYBOARD_INIT_DONE = true; return; }
        ps2_read_data();
        ps2_send_data(0x02);

        KEYBOARD_INIT_DONE = true;
        crate::dev::console::serial_write("[input] keyboard initialized (Num Lock ON)\n");
    }
}

/// Initialize PS/2 mouse: enable auxiliary port, reset, enable reporting.
pub fn mouse_init() {
    unsafe {
        if MOUSE_INIT_DONE { return; }

        let status = port_io::inb(0x64);
        if status == 0xFF {
            crate::dev::console::serial_write("[input] WARN: no PS/2 controller, mouse init skipped\n");
            MOUSE_INIT_DONE = true;
            return;
        }

        // Enable auxiliary PS/2 port
        if !ps2_send_cmd(0xA8) { MOUSE_INIT_DONE = true; return; }

        // Set mouse defaults
        if !ps2_send_cmd(0xD4) { MOUSE_INIT_DONE = true; return; }
        ps2_send_data(0xF6);
        ps2_read_data();

        // Enable data reporting
        if !ps2_send_cmd(0xD4) { MOUSE_INIT_DONE = true; return; }
        ps2_send_data(0xF4);
        ps2_read_data();

        MOUSE_INIT_DONE = true;
        crate::dev::console::serial_write("[input] mouse initialized\n");
    }
}

static mut KEYBOARD_INIT_DONE: bool = false;
static mut MOUSE_INIT_DONE: bool = false;

/// Update keyboard LEDs (bit 0=ScrollLock, bit 1=NumLock, bit 2=CapsLock).
fn set_keyboard_leds(leds: u8) {
    unsafe {
        if port_io::ps2_wait_input() {
            port_io::outb(0x60, 0xED);
        }
        if port_io::ps2_wait_input() {
            port_io::outb(0x60, leds);
        }
    }
}

static mut LED_STATE: u8 = 0x02; // Num Lock ON by default

// ── Keyboard ─────────────────────────────────────────────────────

pub fn poll_key() -> u8 {
    unsafe { keyboard_init(); }
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
        0x2A | 0x36 => { unsafe { SHIFT_HELD = true; } }
        0xAA | 0xB6 => { unsafe { SHIFT_HELD = false; } }
        0x3A => {
            unsafe {
                LED_STATE ^= 0x04;
                set_keyboard_leds(LED_STATE);
            }
        }
        // Ctrl+Shift+Enter → system reboot
        0x1C => {
            unsafe {
                if CTRL_HELD && SHIFT_HELD {
                    crate::dev::console::serial_write("[desktop] Ctrl+Shift+Enter → system reset\n");
                    crate::port_io::system_reset();
                }
            }
        }
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
    unsafe { mouse_init(); }
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
