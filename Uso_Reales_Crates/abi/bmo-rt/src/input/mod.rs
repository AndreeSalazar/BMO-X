//! Input drivers — keyboard + mouse via PS/2.
//!
//! Uses direct x86 `in`/`out` instructions (Ring 0 / CPL=0).
//! When modules move to Ring 3, switch to SYS_PORT_IN/SYS_PORT_OUT.

pub mod ps2;

/// Device-agnostic input event.
#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    KeyDown { scancode: u8, key: char },
    KeyUp { scancode: u8 },
    MouseMove { dx: i16, dy: i16 },
    MouseButton { left: bool, right: bool, middle: bool },
    MouseWheel { delta: i8 },
}

/// Initialize input subsystem (PS/2 keyboard + mouse).
pub fn init() {
    ps2::keyboard_init();
    ps2::mouse_init();
}

/// Poll for any input event. Returns None if no data available.
pub fn poll() -> Option<InputEvent> {
    ps2::keyboard_poll().or_else(|| ps2::mouse_poll())
}
