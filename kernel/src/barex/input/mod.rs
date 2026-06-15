//! `barex::input` — `bx_input`, subsistema de input sobre BMO ABI.
//!
//! Spec: `BareX_Input_Spec.md`.
//!
//! ## Lo que **NO existe** aquí (eliminado por construcción)
//!
//! | Bloat eliminado            | Reemplazo BMO                          |
//! |----------------------------|----------------------------------------|
//! | RawInput (`WM_INPUT`)      | `hid_raw::*` directo desde xHCI        |
//! | DirectInput 8 COM          | `device::DeviceKind` + `gamepad::*`    |
//! | XInput (4 gamepads cap)    | `gamepad::*` ilimitado                 |
//! | Windows.Gaming.Input WinRT | `gamepad::xbox`/`ps`/`switch`          |
//! | HKL / `LoadKeyboardLayout` | `keymap::Layout` portable              |
//! | `GetAsyncKeyState`         | `event::InputReading` snapshot         |
//! | WndProc message loop       | `ring::` SQ/CQ < 0.5 ms                |
//! | mouse acceleration kernel  | `mouse::Reading` raw deltas always     |
//! | Win32 IME / TSF stack      | (no implementado — IME futuro nativo)  |
//!
//! ## Estructura modular (Sesión 12) — sin monolitos
//!
//! ```
//!   input/
//!   ├── mod.rs            ← este archivo (re-exports + versión)
//!   ├── capabilities.rs   ← InputCapabilities bitflags
//!   ├── system.rs         ← BxInputSystem singleton
//!   ├── device/           ← DeviceKind, DeviceInfo
//!   ├── keyboard/         ← Key (USB HID Usage 0x07), Modifiers, Reading
//!   ├── mouse/            ← Buttons, Reading, CursorMode (raw deltas)
//!   ├── headset/          ← HeadsetButton del Redragon
//!   ├── gamepad/          ← Xbox + PlayStation + Switch button maps
//!   ├── wheel/            ← Wheel + HOTAS + FlightStick
//!   ├── hid_raw/          ← Usage Page parser (HID report descriptor)
//!   ├── keymap/           ← Layout US/ES/DVORAK, traducción
//!   ├── event/            ← InputReading snapshot + event types
//!   └── ring/             ← SQ/CQ input low-latency (<0.5 ms)
//! ```

#![allow(dead_code)]

pub mod capabilities;
pub mod system;
pub mod device;
pub mod keyboard;
pub mod mouse;
pub mod headset;
pub mod gamepad;
pub mod wheel;
pub mod hid_raw;
pub mod keymap;
pub mod event;
pub mod ring;

// ─── Re-exports planos ───────────────────────────────────────────────

/// Versión del subsistema `bx_input` (ABI estable a Ring 3).
pub const BX_INPUT_VERSION: (u8, u8, u8) = (1, 0, 0);
