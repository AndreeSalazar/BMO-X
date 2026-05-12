//! `barex::input` — `bx_input`, subsistema de input de FastOS.
//!
//! Spec: `BareX_Input_Spec.md`. Polling HID directo < 0.5 ms vía xHCI.
//! Soporta teclado, ratón, gamepads (Xbox/PS/Switch/Steam Deck), volantes,
//! HOTAS, VR via OpenXR. Sin DirectInput, sin MMSystem joy*, sin
//! coalescing de WM_INPUT, sin "Enhance pointer precision".

use crate::barex::{BxError, BxResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
    GamepadXbox,
    GamepadPlayStation,
    GamepadSwitch,
    Wheel,
    FlightStick,
    Hotas,
    VrController,
    Tablet,
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceId(pub u32);

#[derive(Debug, Clone, Copy)]
pub enum CursorMode {
    Visible,
    Hidden,
    /// Sin cursor visible, deltas raw — modo FPS.
    Captured,
    /// Cursor visible pero confinado al rect de la ventana.
    Confined,
}

/// Snapshot inmutable de input para un frame (modelo `reading` recomendado).
pub struct InputReading {
    pub frame_index: u64,
    pub timestamp_ns: u64,
}

pub struct BxInputSystem {
    _private: (),
}

impl BxInputSystem {
    pub fn instance() -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    pub fn poll(&self) -> BxResult<InputReading> {
        Err(BxError::NotImplemented)
    }
}
