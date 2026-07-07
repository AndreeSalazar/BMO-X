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

/// Full PS/2 controller initialization: enable ports, configure interrupts.
/// Must be called once before keyboard_init() or mouse_init().
fn ps2_controller_init() {
    unsafe {
        if PS2_CTRL_DONE { return; }
        let status = port_io::inb(0x64);
        if status == 0xFF { PS2_CTRL_DONE = true; return; }
        PS2_CTRL_DONE = true;

        // 1. Disable both PS/2 ports
        ps2_send_cmd(0xAD); // disable port 1 (keyboard)
        ps2_send_cmd(0xA7); // disable port 2 (mouse)

        // 2. Flush output buffer (read until empty)
        while let Some(_) = ps2_read_data() {}

        // 3. Read controller configuration byte
        ps2_send_cmd(0x20); // read config
        let config = match ps2_read_data() {
            Some(b) => b,
            None => {
                crate::dev::console::serial_write("[input] WARN: PS/2 config read failed\n");
                return;
            }
        };

        // 4. Modify config: enable IRQ1 (bit 0), enable IRQ12 (bit 1),
        //    disable translation (bit 6 = 0), system flag (bit 2 = 0 for boot)
        let new_config = (config & !0x40) | 0x01; // clear translation, enable IRQ1
        // Don't enable IRQ12 until mouse port is confirmed working

        // 5. Write config back
        ps2_send_cmd(0x60);
        ps2_send_data(new_config);

        // 6. Controller self-test
        ps2_send_cmd(0xAA);
        let test = ps2_read_data();
        let ok = test == Some(0x55);
        crate::dev::console::serial_write("[input] PS/2 self-test: ");
        if ok {
            crate::dev::console::serial_write("OK\n");
        } else {
            crate::dev::console::serial_write("FAIL (");
            crate::dev::console::serial_write_u64(test.unwrap_or(0) as u64, 16);
            crate::dev::console::serial_write(")\n");
        }

        // 7. Enable port 1 (keyboard)
        ps2_send_cmd(0xAE);
        crate::dev::console::serial_write("[input] PS/2 port 1 enabled (keyboard)\n");

        // 8. Try enabling port 2 (mouse) — may fail on single-port controllers
        ps2_send_cmd(0xA8);
        // Read config again to verify dual port
        ps2_send_cmd(0x20);
        let config2 = ps2_read_data().unwrap_or(0);
        if (config2 & 0x20) != 0 {
            // Bit 5 set → second port exists (clock line from mouse)
            // Enable IRQ12
            ps2_send_cmd(0x60);
            ps2_send_data(new_config | 0x02);
            crate::dev::console::serial_write("[input] PS/2 port 2 enabled (mouse)\n");
            PS2_HAS_MOUSE = true;
        } else {
            crate::dev::console::serial_write("[input] PS/2 single-port (no mouse port)\n");
        }
    }
}

static mut PS2_CTRL_DONE: bool = false;
static mut PS2_HAS_MOUSE: bool = false;

// ── PS/2 I/O helpers ─────────────────────────────────────────────

unsafe fn ps2_send_cmd(cmd: u8) -> bool {
    if !port_io::ps2_wait_input() { return false; }
    port_io::outb(0x64, cmd);
    true
}

unsafe fn ps2_send_data(data: u8) -> bool {
    if !port_io::ps2_wait_input() { return false; }
    port_io::outb(0x60, data);
    true
}

unsafe fn ps2_read_data() -> Option<u8> {
    for _ in 0..5000 {
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
        ps2_controller_init();

        let status = port_io::inb(0x64);
        if status == 0xFF { KEYBOARD_INIT_DONE = true; return; }

        // Drain stale data
        while let Some(_) = ps2_read_data() {}

        // Reset keyboard (0xFF), wait for BAT result
        ps2_send_data(0xFF);
        ps2_read_data(); // ACK
        let bat = ps2_read_data();
        // Enable scanning
        ps2_send_data(0xF4);
        ps2_read_data();
        // Num Lock LED on
        ps2_send_data(0xED);
        ps2_read_data();
        ps2_send_data(0x02);

        KEYBOARD_INIT_DONE = true;
        crate::dev::console::serial_write("[input] keyboard ready (BAT=");
        crate::dev::console::serial_write_u64(bat.unwrap_or(0) as u64, 16);
        crate::dev::console::serial_write(")\n");
    }
}

pub fn mouse_init() {
    unsafe {
        if MOUSE_INIT_DONE { return; }
        ps2_controller_init();
        if !PS2_HAS_MOUSE { MOUSE_INIT_DONE = true; return; }

        ps2_send_cmd(0xD4); ps2_send_data(0xFF); // reset
        ps2_read_data(); // ACK
        let bat = ps2_read_data();
        ps2_read_data(); // device ID

        ps2_send_cmd(0xD4); ps2_send_data(0xF6); // defaults
        ps2_read_data();
        ps2_send_cmd(0xD4); ps2_send_data(0xF4); // enable
        ps2_read_data();

        MOUSE_INIT_DONE = true;
        crate::dev::console::serial_write("[input] mouse ready (BAT=");
        crate::dev::console::serial_write_u64(bat.unwrap_or(0) as u64, 16);
        crate::dev::console::serial_write(")\n");
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
