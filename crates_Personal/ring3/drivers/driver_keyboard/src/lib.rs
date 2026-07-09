//! Keyboard Driver — reads scancodes via SYS_KEYBOARD_POLL.
//!
//! The kernel's PS/2 ISR pushes scancodes into the system BMO Channel.
//! This driver reads them via a lightweight syscall (~200ns/event).

#![no_std]

extern crate alloc;

use ring3_foundation;

/// Keyboard event.
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub scancode: u8,
    pub pressed: bool,
}

/// Poll for next keyboard event. Returns None if no events pending.
pub fn poll() -> Option<KeyEvent> {
    let sc = ring3_foundation::sys_keyboard_poll();
    if sc == 0 { return None; }
    Some(KeyEvent {
        scancode: sc & 0x7F,
        pressed: (sc & 0x80) == 0,
    })
}
