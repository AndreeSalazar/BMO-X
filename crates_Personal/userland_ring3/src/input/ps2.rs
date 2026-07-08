//! PS/2 keyboard + mouse driver via port 0x60/0x64.
//!
//! ## Ports
//! - 0x60: data port (read = data from device, write = data to device)
//! - 0x64: status (read) / command (write)
//!   - Status bit 0: output buffer full (data available at 0x60)
//!   - Status bit 1: input buffer full (device busy, wait)
//!
//! ## Keyboard
//! - IRQ1, scancode set 1
//! - Scancodes translated to ASCII via US QWERTY layout
//! - Keys: press (make) / release (break = make | 0x80)
//!
//! ## Mouse
//! - IRQ12, 3-byte packets
//! - Packet: [flags(bit3=1, yov,xov,signY,signX,mid,right,left), dx, dy]
//! - Must be enabled via PS/2 controller command sequence

use core::arch::asm;

// ── Port I/O primitives (direct for Ring 0) ──────────────────────────

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!("in al, dx", in("dx") port, out("al") val, options(nostack, nomem));
    val
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nostack, nomem));
}

#[inline]
unsafe fn io_wait() {
    outb(0x80, 0);
}

// ── PS/2 controller commands ──────────────────────────────────────────

unsafe fn ps2_read_status() -> u8 {
    inb(0x64)
}

unsafe fn ps2_wait_write() {
    while ps2_read_status() & 0x02 != 0 {
        core::hint::spin_loop();
    }
}

unsafe fn ps2_wait_read() {
    for _ in 0..100_000 {
        if ps2_read_status() & 0x01 != 0 {
            return;
        }
        core::hint::spin_loop();
    }
}

unsafe fn ps2_write_cmd(cmd: u8) {
    ps2_wait_write();
    outb(0x64, cmd);
}

unsafe fn ps2_write_data(data: u8) {
    ps2_wait_write();
    outb(0x60, data);
}

unsafe fn ps2_read_data() -> u8 {
    ps2_wait_read();
    inb(0x60)
}

unsafe fn ps2_has_data() -> bool {
    ps2_read_status() & 0x01 != 0
}

// ── Keyboard ──────────────────────────────────────────────────────────

/// US QWERTY scancode-to-ASCII (set 1, make codes).
static KEY_MAP: [char; 128] = [
    '\0',   '\x1B', '1',  '2',  '3',  '4',  '5',  '6',   // 0x00-0x07
    '7',    '8',    '9',  '0',  '-',  '=',  '\x08','\t', // 0x08-0x0F
    'q',    'w',    'e',  'r',  't',  'y',  'u',  'i',   // 0x10-0x17
    'o',    'p',    '[',  ']',  '\n', '\0', 'a',  's',   // 0x18-0x1F
    'd',    'f',    'g',  'h',  'j',  'k',  'l',  ';',   // 0x20-0x27
    '\'',   '`',    '\0', '\\', 'z',  'x',  'c',  'v',   // 0x28-0x2F
    'b',    'n',    'm',  ',',  '.',  '/',  '\0', '*',   // 0x30-0x37
    '\0',   ' ',    '\0', '\0', '\0', '\0', '\0', '\0',   // 0x38-0x3F
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x40-0x47
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x48-0x4F
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x50-0x57
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x58-0x5F
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x60-0x67
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x68-0x6F
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x70-0x77
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x78-0x7F
];

/// Shift-modifier map for US QWERTY.
static KEY_SHIFT: [char; 128] = [
    '\0',   '\0',   '!',  '@',  '#',  '$',  '%',  '^',   // 0x00-0x07
    '&',    '*',    '(',  ')',  '_',  '+',  '\x08','\t', // 0x08-0x0F
    'Q',    'W',    'E',  'R',  'T',  'Y',  'U',  'I',   // 0x10-0x17
    'O',    'P',    '{',  '}',  '\n', '\0', 'A',  'S',   // 0x18-0x1F
    'D',    'F',    'G',  'H',  'J',  'K',  'L',  ':',   // 0x20-0x27
    '"',    '~',    '\0', '|',  'Z',  'X',  'C',  'V',   // 0x28-0x2F
    'B',    'N',    'M',  '<',  '>',  '?',  '\0', '*',   // 0x30-0x37
    '\0',   ' ',    '\0', '\0', '\0', '\0', '\0', '\0',   // 0x38-0x3F
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x40-0x47
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x48-0x4F
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x50-0x57
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x58-0x5F
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x60-0x67
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x68-0x6F
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x70-0x77
    '\0',   '\0',   '\0', '\0', '\0', '\0', '\0', '\0',   // 0x78-0x7F
];

static mut SHIFT_PRESSED: bool = false;

pub fn keyboard_init() {
    // PS/2 keyboard is auto-detected by BIOS. Enable scanning.
    unsafe {
        // Tell keyboard to enable scanning
        ps2_write_cmd(0xAE); // enable first PS/2 port
        io_wait();
        ps2_write_data(0xF4); // enable scanning
        let ack = ps2_read_data();
        if ack != 0xFA {
            // Keyboard didn't ACK — might not be present
        }
    }
}

pub fn keyboard_poll() -> Option<super::InputEvent> {
    unsafe {
        if !ps2_has_data() { return None; }
        let sc = ps2_read_data();

        if sc == 0xE0 {
            // Extended prefix — read next byte
            if !ps2_has_data() { return None; }
            let ext = ps2_read_data();
            return match ext {
                0x48 => Some(super::InputEvent::KeyDown { scancode: ext, key: '\0' }), // Up
                0x50 => Some(super::InputEvent::KeyDown { scancode: ext, key: '\0' }), // Down
                0x4B => Some(super::InputEvent::KeyDown { scancode: ext, key: '\0' }), // Left
                0x4D => Some(super::InputEvent::KeyDown { scancode: ext, key: '\0' }), // Right
                _ => None,
            };
        }

        if sc & 0x80 != 0 {
            // Key release
            let base = sc & !0x80;
            if base == 0x2A || base == 0x36 {
                SHIFT_PRESSED = false;
            }
            return Some(super::InputEvent::KeyUp { scancode: base });
        }

        // Key press
        if sc == 0x2A || sc == 0x36 {
            SHIFT_PRESSED = true;
            return None; // shift press produces no character
        }

        let shift = SHIFT_PRESSED;
        let key = if (sc as usize) < 128 {
            if shift { KEY_SHIFT[sc as usize] } else { KEY_MAP[sc as usize] }
        } else {
            '\0'
        };

        if key == '\0' { return None; }
        Some(super::InputEvent::KeyDown { scancode: sc, key })
    }
}

// ── Mouse ─────────────────────────────────────────────────────────────

static mut MOUSE_BYTES: [u8; 3] = [0; 3];
static mut MOUSE_IDX: usize = 0;
static mut MOUSE_CYCLE: u8 = 0;

pub fn mouse_init() {
    unsafe {
        // Enable auxiliary PS/2 port (mouse)
        ps2_write_cmd(0xA8);
        io_wait();

        // Get Compaq status byte, set bit 1 (mouse IRQ12 enable) and bit 5 (mouse clock)
        ps2_write_cmd(0x20);
        let mut cfg = ps2_read_data();
        cfg |= 0x02; // enable mouse IRQ12
        cfg &= !0x20; // disable mouse clock
        io_wait();

        // Write back config
        ps2_write_cmd(0x60);
        ps2_write_data(cfg);
        io_wait();

        // Send "enable data reporting" to mouse
        ps2_write_cmd(0xD4); // next data goes to mouse
        ps2_write_data(0xF4); // enable data reporting
        let ack = ps2_read_data();
        if ack != 0xFA {
            // Mouse didn't ACK
        }
    }
}

pub fn mouse_poll() -> Option<super::InputEvent> {
    unsafe {
        if !ps2_has_data() { return None; }
        let b = ps2_read_data();

        MOUSE_CYCLE = MOUSE_CYCLE.wrapping_add(1);

        match MOUSE_CYCLE {
            1 => {
                MOUSE_BYTES[0] = b;
                // Bit 3 must be 1 for valid packet
                if b & 0x08 == 0 {
                    MOUSE_CYCLE = 0;
                }
                None
            }
            2 => {
                MOUSE_BYTES[1] = b;
                None
            }
            _ => { // 3+
                MOUSE_BYTES[2] = b;
                MOUSE_CYCLE = 0;

                let flags = MOUSE_BYTES[0];
                let dx = if flags & 0x10 != 0 {
                    (MOUSE_BYTES[1] as i8) as i16
                } else {
                    MOUSE_BYTES[1] as i16
                };
                let dy = if flags & 0x20 != 0 {
                    -((MOUSE_BYTES[2] as i8) as i16)
                } else {
                    -(MOUSE_BYTES[2] as i16)
                };
                let left = flags & 0x01 != 0;
                let right = flags & 0x02 != 0;
                let middle = flags & 0x04 != 0;

                if dx != 0 || dy != 0 {
                    Some(super::InputEvent::MouseMove { dx, dy })
                } else if left || right || middle {
                    Some(super::InputEvent::MouseButton { left, right, middle })
                } else {
                    None
                }
            }
        }
    }
}
