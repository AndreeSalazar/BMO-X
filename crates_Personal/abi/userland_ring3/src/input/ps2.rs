//! PS/2 keyboard + mouse driver — Ring 3 stub.
//!
//! In Ring 3, direct port I/O (`in`/`out`) causes #GP.
//! The kernel's PS/2 driver handles hardware access via IRQs.
//! This module provides scancode translation and packet parsing
//! for input events delivered by the kernel via the input service.

use bmo_abi::syscalls;

// ── Ring 3 input polling via syscalls ──────────────────────────────────

/// Poll for a keyboard event from the kernel input service.
fn kernel_poll_key() -> Option<u8> {
    let result = unsafe { syscalls::syscall0(syscalls::NR_INPUT_POLL_KEY) };
    if result.is_ok() {
        let raw = result.value() as u8;
        if raw != 0 { Some(raw) } else { None }
    } else {
        None
    }
}

/// Poll for a mouse event from the kernel input service.
/// Returns (delta_x, delta_y, buttons) packed into a u64.
fn kernel_poll_mouse() -> Option<(i16, i16, u8)> {
    let result = unsafe { syscalls::syscall0(syscalls::NR_INPUT_POLL_MOUSE) };
    if result.is_ok() {
        let raw = result.value();
        let dx = (raw as i16) << 8 >> 8;   // sign-extend from low 16 bits
        let dy = ((raw >> 16) as i16) << 8 >> 8;
        let buttons = ((raw >> 32) & 0xFF) as u8;
        Some((dx, dy, buttons))
    } else {
        None
    }
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
    // Ring 3: kernel handles PS/2 controller init.
    // No-op here; the kernel's PS/2 driver enables scanning on boot.
}

pub fn keyboard_poll() -> Option<super::InputEvent> {
    let sc = kernel_poll_key()?;

    // Extended prefix (E0): arrow keys, etc.
    if sc == 0xE0 {
        let ext = kernel_poll_key()?;
        return match ext {
            0x48 => Some(super::InputEvent::KeyDown { scancode: ext, key: '\0' }),
            0x50 => Some(super::InputEvent::KeyDown { scancode: ext, key: '\0' }),
            0x4B => Some(super::InputEvent::KeyDown { scancode: ext, key: '\0' }),
            0x4D => Some(super::InputEvent::KeyDown { scancode: ext, key: '\0' }),
            _ => None,
        };
    }

    // Key release (break code)
    if sc & 0x80 != 0 {
        let base = sc & !0x80;
        if base == 0x2A || base == 0x36 {
            unsafe { SHIFT_PRESSED = false; }
        }
        return Some(super::InputEvent::KeyUp { scancode: base });
    }

    // Shift press
    if sc == 0x2A || sc == 0x36 {
        unsafe { SHIFT_PRESSED = true; }
        return None;
    }

    let shift = unsafe { SHIFT_PRESSED };
    let key = if (sc as usize) < 128 {
        if shift { KEY_SHIFT[sc as usize] } else { KEY_MAP[sc as usize] }
    } else {
        '\0'
    };

    if key == '\0' { return None; }
    Some(super::InputEvent::KeyDown { scancode: sc, key })
}

// ── Mouse ─────────────────────────────────────────────────────────────

pub fn mouse_init() {
    // Ring 3: kernel handles PS/2 mouse init (enable auxiliary port, data reporting).
    // No-op here.
}

pub fn mouse_poll() -> Option<super::InputEvent> {
    let (dx, dy, buttons) = kernel_poll_mouse()?;

    let left = buttons & 0x01 != 0;
    let right = buttons & 0x02 != 0;
    let middle = buttons & 0x04 != 0;

    if dx != 0 || dy != 0 {
        Some(super::InputEvent::MouseMove { dx, dy })
    } else if left || right || middle {
        Some(super::InputEvent::MouseButton { left, right, middle })
    } else {
        None
    }
}
