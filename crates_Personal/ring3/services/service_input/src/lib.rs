//! Input Service — unified keyboard + mouse event stream.
//!
//! Consumes keyboard and mouse drivers, produces a single stream
//! of typed `InputEvent`s that the desktop can consume.
//!
//! ## Usage
//!
//! ```rust
//! let mut input = InputService::connect(sys_channel_phys);
//! loop {
//!     for event in input.poll() {
//!         match event {
//!             InputEvent::Key { scancode, pressed } => { ... }
//!             InputEvent::MouseMove { dx, dy } => { ... }
//!             InputEvent::MouseButton { buttons } => { ... }
//!         }
//!     }
//! }
//! ```

#![no_std]

extern crate alloc;

use driver_keyboard::{Keyboard, KeyEvent};
use driver_mouse::{Mouse, MouseState};

/// Unified input event.
#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Key { scancode: u8, pressed: bool },
    MouseMove { dx: i64, dy: i64 },
    MouseButton { buttons: u8 },
}

/// Input service — owns keyboard and mouse drivers.
pub struct InputService {
    keyboard: Keyboard,
    mouse: Mouse,
}

impl InputService {
    /// Connect to the system BMO Channel.
    pub fn connect(sys_channel_phys: u64) -> Self {
        Self {
            keyboard: Keyboard::connect(sys_channel_phys),
            mouse: Mouse::connect(sys_channel_phys),
        }
    }

    /// Poll for all input events since last call.
    pub fn poll(&mut self) -> alloc::vec::Vec<InputEvent> {
        let mut events = alloc::vec::Vec::new();

        // Keyboard events
        for kev in self.keyboard.poll() {
            events.push(InputEvent::Key {
                scancode: kev.scancode,
                pressed: kev.pressed,
            });
        }

        // Mouse events
        if let Some(ms) = self.mouse.poll() {
            if ms.dx != 0 || ms.dy != 0 {
                events.push(InputEvent::MouseMove { dx: ms.dx, dy: ms.dy });
            }
            events.push(InputEvent::MouseButton { buttons: ms.buttons });
        }

        events
    }
}
