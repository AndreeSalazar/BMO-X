//! `barex::input` — `bx_input`, subsistema de input sobre BMO ABI.
//!
//! Spec: `BareX_Input_Spec.md`.
//!
//! ## Devices realmente conectados a este equipo
//!
//! - **Teclado USB** → `drivers::usb::hid` (Boot Protocol o Report Protocol).
//! - **Ratón USB** → `drivers::usb::hid` con report HighRes (16-bit deltas).
//! - **Headset Redragon** → botones de volumen/mute como Consumer Page HID.
//!
//! Los gamepads, volantes, HOTAS y VR de la spec están **declarados pero no
//! activos** en este equipo (se enchufarían y serían reconocidos vía xHCI).

#![allow(dead_code)]

use crate::barex::{BxError, BxResult};
use crate::barex::abi::BmoHandle;

// ═══════════════════════════════════════════════════════════════════════
//   Devices y identificación
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
    HeadsetButtons,
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
pub struct DeviceInfo {
    pub handle: BmoHandle,
    pub kind: DeviceKind,
    pub vendor_id: u16,
    pub product_id: u16,
    pub poll_rate_hz: u32,
}

// ═══════════════════════════════════════════════════════════════════════
//   Teclado — keycodes USB HID Usage Page 0x07 (universal)
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Key {
    None = 0x00,
    A = 0x04, B = 0x05, C = 0x06, D = 0x07, E = 0x08, F = 0x09,
    G = 0x0A, H = 0x0B, I = 0x0C, J = 0x0D, K = 0x0E, L = 0x0F,
    M = 0x10, N = 0x11, O = 0x12, P = 0x13, Q = 0x14, R = 0x15,
    S = 0x16, T = 0x17, U = 0x18, V = 0x19, W = 0x1A, X = 0x1B,
    Y = 0x1C, Z = 0x1D,
    N1 = 0x1E, N2 = 0x1F, N3 = 0x20, N4 = 0x21, N5 = 0x22,
    N6 = 0x23, N7 = 0x24, N8 = 0x25, N9 = 0x26, N0 = 0x27,
    Enter = 0x28, Escape = 0x29, Backspace = 0x2A, Tab = 0x2B, Space = 0x2C,
    F1 = 0x3A, F2 = 0x3B, F3 = 0x3C, F4 = 0x3D, F5 = 0x3E, F6 = 0x3F,
    F7 = 0x40, F8 = 0x41, F9 = 0x42, F10 = 0x43, F11 = 0x44, F12 = 0x45,
    Right = 0x4F, Left = 0x50, Down = 0x51, Up = 0x52,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Modifiers: u8 {
        const L_CTRL  = 1 << 0;
        const L_SHIFT = 1 << 1;
        const L_ALT   = 1 << 2;
        const L_GUI   = 1 << 3;
        const R_CTRL  = 1 << 4;
        const R_SHIFT = 1 << 5;
        const R_ALT   = 1 << 6;
        const R_GUI   = 1 << 7;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KeyboardReading {
    pub modifiers: Modifiers,
    pub keys_down: [Key; 6],
}

// ═══════════════════════════════════════════════════════════════════════
//   Ratón — alta resolución (16-bit deltas + wheel V/H)
// ═══════════════════════════════════════════════════════════════════════

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct MouseButtons: u8 {
        const LEFT     = 1 << 0;
        const RIGHT    = 1 << 1;
        const MIDDLE   = 1 << 2;
        const BACK     = 1 << 3;
        const FORWARD  = 1 << 4;
        const EXTRA1   = 1 << 5;
        const EXTRA2   = 1 << 6;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MouseReading {
    pub buttons: MouseButtons,
    /// Delta X raw del último poll. Sin aceleración del SO.
    pub dx: i16,
    pub dy: i16,
    pub wheel_v: i8,
    pub wheel_h: i8,
}

// ═══════════════════════════════════════════════════════════════════════
//   Headset (botones del Redragon)
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadsetButton {
    VolumeUp,
    VolumeDown,
    Mute,
    MicMute,
    PlayPause,
    NextTrack,
    PrevTrack,
}

#[derive(Debug, Clone, Copy)]
pub struct HeadsetButtonEvent {
    pub button: HeadsetButton,
    pub pressed: bool,
}

// ═══════════════════════════════════════════════════════════════════════
//   Cursor mode
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
pub enum CursorMode {
    Visible,
    Hidden,
    /// Sin cursor visible, deltas raw — modo FPS.
    Captured,
    /// Cursor visible pero confinado al rect de la ventana.
    Confined,
}

// ═══════════════════════════════════════════════════════════════════════
//   Snapshot por frame (modelo recomendado)
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
pub struct InputReading {
    pub frame_index: u64,
    pub timestamp_ns: u64,
    pub keyboard: KeyboardReading,
    pub mouse: MouseReading,
}

impl InputReading {
    pub const EMPTY: Self = Self {
        frame_index: 0,
        timestamp_ns: 0,
        keyboard: KeyboardReading {
            modifiers: Modifiers::empty(),
            keys_down: [Key::None; 6],
        },
        mouse: MouseReading {
            buttons: MouseButtons::empty(),
            dx: 0, dy: 0, wheel_v: 0, wheel_h: 0,
        },
    };

    pub fn key_down(&self, k: Key) -> bool {
        self.keyboard.keys_down.iter().any(|&x| x as u8 == k as u8)
    }
}

// ═══════════════════════════════════════════════════════════════════════
//   API pública
// ═══════════════════════════════════════════════════════════════════════

pub struct BxInputSystem {
    _private: (),
}

impl BxInputSystem {
    /// Singleton — el HID service de Ring 3 vive detrás de esto.
    pub fn instance() -> BxResult<Self> {
        // TODO: conectarse a `drivers::usb::hid::poll_events`.
        Err(BxError::NotImplemented)
    }

    pub fn poll(&self) -> BxResult<InputReading> {
        Err(BxError::NotImplemented)
    }

    pub fn set_cursor_mode(&self, _mode: CursorMode) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
