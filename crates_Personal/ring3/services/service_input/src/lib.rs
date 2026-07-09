//! Input Service — unified keyboard + mouse event stream.
//!
//! Consumes keyboard and mouse drivers via syscall-based polling.
//! Produces a single stream of typed `InputEvent`s for the desktop.

#![no_std]

extern crate alloc;

use driver_keyboard;
use driver_mouse;

/// Unified input event.
#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Key { scancode: u8, pressed: bool },
    MouseMove { dx: i16, dy: i16 },
    MouseButton { buttons: u8 },
}

/// Poll for all pending input events.
pub fn poll() -> alloc::vec::Vec<InputEvent> {
    let mut events = alloc::vec::Vec::new();

    // Keyboard
    while let Some(kev) = driver_keyboard::poll() {
        events.push(InputEvent::Key { scancode: kev.scancode, pressed: kev.pressed });
    }

    // Mouse
    if let Some(ms) = driver_mouse::poll() {
        if ms.dx != 0 || ms.dy != 0 {
            events.push(InputEvent::MouseMove { dx: ms.dx, dy: ms.dy });
        }
        events.push(InputEvent::MouseButton { buttons: ms.buttons });
    }

    events
}
