//! Input Service — unified keyboard + mouse event stream.
//!
//! Consumes keyboard and mouse drivers via syscall-based polling.
//! Produces typed `InputEvent`s for the desktop.
//!
//! Two APIs: `poll()` (heap-allocating, convenient) and
//! `poll_into()` (stack-only, zero-allocation for 60fps loops).

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

/// Maximum events per frame. Exceeding this indicates a stuck key or
/// hardware issue. The buffer is stack-allocated by the caller.
pub const MAX_EVENTS_PER_FRAME: usize = 16;

/// Poll for all pending input events (heap-allocating, convenient).
pub fn poll() -> alloc::vec::Vec<InputEvent> {
    let mut events = alloc::vec::Vec::new();
    while let Some(kev) = driver_keyboard::poll() {
        events.push(InputEvent::Key { scancode: kev.scancode, pressed: kev.pressed });
    }
    if let Some(ms) = driver_mouse::poll() {
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
            if count < buf.len() {
                buf[count] = InputEvent::Key { scancode: kev.scancode, pressed: kev.pressed };
                count += 1;
            }
        } else { break; }
    }
    if count < buf.len() {
        if let Some(ms) = driver_mouse::poll() {
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
