#![no_std]

extern crate alloc;

use driver_keyboard;
use driver_mouse;
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Key { scancode: u8, pressed: bool },
    MouseMove { dx: i16, dy: i16 },
    MouseButton { buttons: u8 },
}

pub const MAX_EVENTS_PER_FRAME: usize = 16;

/// Keyboard modifier state (atomic, visible to layout translation).
static KBD_LSHIFT: AtomicBool = AtomicBool::new(false);
static KBD_RSHIFT: AtomicBool = AtomicBool::new(false);
static KBD_CTRL:  AtomicBool = AtomicBool::new(false);
static KBD_ALT:   AtomicBool = AtomicBool::new(false);
static KBD_CAPS:  AtomicBool = AtomicBool::new(false);

static MOUSE_X: AtomicI32 = AtomicI32::new(0);
static MOUSE_Y: AtomicI32 = AtomicI32::new(0);
static ESC_LATCH: AtomicBool = AtomicBool::new(false);

#[inline]
pub fn shift_held() -> bool { KBD_LSHIFT.load(Ordering::Relaxed) || KBD_RSHIFT.load(Ordering::Relaxed) }
#[inline]
pub fn caps_on() -> bool { KBD_CAPS.load(Ordering::Relaxed) }
#[inline]
pub fn alt_held() -> bool { KBD_ALT.load(Ordering::Relaxed) }
#[inline]
pub fn ctrl_held() -> bool { KBD_CTRL.load(Ordering::Relaxed) }
#[inline]
pub fn mouse_x() -> i32 { MOUSE_X.load(Ordering::Relaxed) }
#[inline]
pub fn mouse_y() -> i32 { MOUSE_Y.load(Ordering::Relaxed) }

/// Translate PS/2 Set 1 scancode to virtual key code.
/// Updates internal modifier state (shift, ctrl, alt, caps).
/// Returns None for modifier-only keys, Some((kind, vk)) for all others.
pub fn translate_scancode(sc_raw: u8) -> Option<(/* KeyUp/Down flag */ bool, /* VK */ u8)> {
    let released = (sc_raw & 0x80) != 0;
    let sc = sc_raw & 0x7F;
    match sc {
        0x2A => { KBD_LSHIFT.store(!released, Ordering::Relaxed); return None; }
        0x36 => { KBD_RSHIFT.store(!released, Ordering::Relaxed); return None; }
        0x1D => { KBD_CTRL.store(!released, Ordering::Relaxed); return None; }
        0x38 => { KBD_ALT.store(!released, Ordering::Relaxed); return None; }
        0x3A => {
            if !released { KBD_CAPS.store(!KBD_CAPS.load(Ordering::Relaxed), Ordering::Relaxed); }
            return None;
        }
        _ => {}
    }
    let vk = match sc {
        0x01 => { ESC_LATCH.store(true, Ordering::Relaxed); 0x1B }
        0x0E => 0x08, 0x0F => 0x09, 0x1C => 0x0D, 0x39 => 0x20,
        0x4B => 0x25, 0x48 => 0x26, 0x4D => 0x27, 0x50 => 0x28,
        0x47 => 0x24, 0x4F => 0x23, 0x49 => 0x21, 0x51 => 0x22,
        0x52 => 0x2D, 0x53 => 0x2E,
        0x3B => 0x70, 0x3C => 0x71, 0x3D => 0x72, 0x3E => 0x73,
        0x3F => 0x74, 0x40 => 0x75, 0x41 => 0x76, 0x42 => 0x77,
        0x43 => 0x78, 0x44 => 0x79, 0x57 => 0x7A, 0x58 => 0x7B,
        0x02 => b'1', 0x03 => b'2', 0x04 => b'3', 0x05 => b'4',
        0x06 => b'5', 0x07 => b'6', 0x08 => b'7', 0x09 => b'8',
        0x0A => b'9', 0x0B => b'0',
        0x1E => b'A', 0x30 => b'B', 0x2E => b'C', 0x20 => b'D',
        0x12 => b'E', 0x21 => b'F', 0x22 => b'G', 0x23 => b'H',
        0x17 => b'I', 0x24 => b'J', 0x25 => b'K', 0x26 => b'L',
        0x32 => b'M', 0x31 => b'N', 0x18 => b'O', 0x19 => b'P',
        0x10 => b'Q', 0x13 => b'R', 0x1F => b'S', 0x14 => b'T',
        0x16 => b'U', 0x2F => b'V', 0x11 => b'W', 0x2D => b'X',
        0x15 => b'Y', 0x2C => b'Z',
        0x1A => b'[', 0x1B => b']', 0x2B => b'\\', 0x27 => b';',
        0x28 => b'\'', 0x33 => b',', 0x34 => b'.', 0x35 => b'/',
        0x29 => b'`', 0x0C => b'-', 0x0D => b'=',
        _ => sc,
    };
    Some((released, vk))
}

/// Spanish keyboard layout: scancode → ASCII character.
/// Returns None for non-character keys (arrows, modifiers, F-keys).
pub fn scancode_to_char_es(sc: u8) -> Option<u8> {
    let released = (sc & 0x80) != 0;
    if released { return None; }
    let sc = sc & 0x7F;
    let upper = caps_on() ^ shift_held();
    match sc {
        0x02..=0x09 => Some(if upper { b'1' - 1 + sc as u8 - 1 } else { b'1' - 1 + sc as u8 - 1 }),
        0x0A => None,
        0x0B => Some(b'0'),
        0x0C => Some(if upper { b'?' } else { b'\'' }),
        0x0D => Some(if upper { 0xA8 } else { 0xAD }),
        0x10 | 0x1E | 0x1F | 0x20 | 0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26
        | 0x11 | 0x12 | 0x13 | 0x14 | 0x15 | 0x16 | 0x17 | 0x18 | 0x19
        | 0x2C | 0x2D | 0x2E | 0x2F | 0x30 | 0x31 | 0x32 => {
            let base: u8 = if upper { b'A' - 0x1E } else { b'a' - 0x1E };
            Some(base + sc as u8)
        }
        0x27 => Some(if upper { 0xA5 } else { 0xA4 }),
        0x33 => Some(if upper { b';' } else { b',' }),
        0x34 => Some(if upper { b':' } else { b'.' }),
        0x35 => Some(if upper { b'_' } else { b'-' }),
        0x39 => Some(b' '),
        0x0E => Some(0x08),
        0x1C => Some(b'\n'),
        0x53 => Some(0x7F),
        _ => None,
    }
}

/// Check if ESC was pressed since last call (latches).
pub fn esc_pressed() -> bool {
    let r = ESC_LATCH.load(Ordering::Relaxed);
    ESC_LATCH.store(false, Ordering::Relaxed);
    r
}

/// Poll for all pending input events (heap-allocating, convenient).
/// Updates internal state (modifiers, mouse position, buttons).
pub fn poll() -> alloc::vec::Vec<InputEvent> {
    let mut events = alloc::vec::Vec::new();
    while let Some(kev) = driver_keyboard::poll() {
        translate_scancode(if kev.pressed { kev.scancode } else { kev.scancode | 0x80 });
        events.push(InputEvent::Key { scancode: kev.scancode, pressed: kev.pressed });
    }
    if let Some(ms) = driver_mouse::poll() {
        MOUSE_X.fetch_add(ms.dx as i32, Ordering::Relaxed);
        MOUSE_Y.fetch_add(ms.dy as i32, Ordering::Relaxed);
        if ms.dx != 0 || ms.dy != 0 {
            events.push(InputEvent::MouseMove { dx: ms.dx, dy: ms.dy });
        }
        events.push(InputEvent::MouseButton { buttons: ms.buttons });
    }
    events
}

/// Poll into a pre-allocated buffer (zero heap allocations).
/// Returns the number of events written. Caps at `buf.len()`.
pub fn poll_into(buf: &mut [InputEvent]) -> usize {
    let mut count = 0;
    while count < buf.len() {
        if let Some(kev) = driver_keyboard::poll() {
            translate_scancode(if kev.pressed { kev.scancode } else { kev.scancode | 0x80 });
            if count < buf.len() {
                buf[count] = InputEvent::Key { scancode: kev.scancode, pressed: kev.pressed };
                count += 1;
            }
        } else { break; }
    }
    if count < buf.len() {
        if let Some(ms) = driver_mouse::poll() {
            MOUSE_X.fetch_add(ms.dx as i32, Ordering::Relaxed);
            MOUSE_Y.fetch_add(ms.dy as i32, Ordering::Relaxed);
            if ms.dx != 0 || ms.dy != 0 {
                if count < buf.len() {
                    buf[count] = InputEvent::MouseMove { dx: ms.dx, dy: ms.dy };
                    count += 1;
                }
            }
            if count < buf.len() {
                buf[count] = InputEvent::MouseButton { buttons: ms.buttons };
                count += 1;
            }
        }
    }
    count
}
