//! `BxInputSystem` — singleton de input. Tras él vive el HID service de Ring 3.

use crate::barex::{BxError, BxResult};
use super::event::InputReading;
use super::mouse::CursorMode;

pub struct BxInputSystem {
    _private: (),
}

impl BxInputSystem {
    /// Devuelve handle al singleton. Conecta a `drivers::usb::hid::poll_events`.
    pub fn instance() -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    /// Snapshot del frame actual.
    pub fn poll(&self) -> BxResult<InputReading> {
        Err(BxError::NotImplemented)
    }

    pub fn set_cursor_mode(&self, _mode: CursorMode) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
