//! InputHal trait -- contract that every input backend must implement.

use crate::event::InputEvent;

/// Whether the device reports absolute or relative pointer positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerMode {
    Relative,  // mouse (dx, dy)
    Absolute,  // touch/pen (x, y)
}

/// Trait implemented by every input hardware backend (PS/2, USB HID, etc.).
pub trait InputHal {
    /// One-time initialization: controller config, device detection, reset.
    fn init(&mut self) -> bool;

    /// Human-readable backend name for diagnostics.
    fn name(&self) -> &'static str;

    /// Non-blocking poll: drain available input events into the buffer.
    /// Returns number of events written. Call from main loop or IRQ handler.
    fn poll(&mut self, buf: &mut [InputEvent]) -> usize;

    /// Whether this backend provides relative or absolute pointer data.
    fn pointer_mode(&self) -> PointerMode { PointerMode::Relative }

    /// Whether the backend has been initialized successfully.
    fn is_ready(&self) -> bool;
}
