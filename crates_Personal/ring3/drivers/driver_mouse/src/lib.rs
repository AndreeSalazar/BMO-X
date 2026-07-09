//! Mouse Driver — reads mouse events via SYS_MOUSE_POLL.
//!
//! The kernel's PS/2 ISR pushes mouse packets into the system BMO Channel.
//! This driver reads them via a lightweight syscall.

#![no_std]

extern crate alloc;

use ring3_foundation;

#[derive(Debug, Clone, Copy, Default)]
pub struct MouseState {
    pub dx: i16,
    pub dy: i16,
    pub buttons: u8,
}

/// Poll for mouse events. Returns the accumulated state or None if unchanged.
pub fn poll() -> Option<MouseState> {
    let packed = ring3_foundation::sys_mouse_poll();
    if packed == u64::MAX { return None; }
    let dx = (packed & 0xFFFF) as i16;
    let dy = ((packed >> 16) & 0xFFFF) as i16;
    let buttons = ((packed >> 32) & 0xFF) as u8;
    Some(MouseState { dx, dy, buttons })
}
