//! `bmo_abi::input` — Eventos de teclado, ratón y gamepad.
//!
//! Define los tipos que `bmo_input_poll_*` y los eventos BEFCore
//! pueden entregar. **Los syscalls reales** están en
//! `crate::bmo_abi::syscalls`.
//!
//! ## Distinción
//!
//! - `BmoInputEvent`: evento genérico leído por polling
//!   (`bmo_input_poll_event`).
//! - Los eventos que llegan vía BEFCore (`BmoKeyEvent`, `BmoMouseEvent`)
//!   viven en `crate::bmo_abi::windowing`.

#![allow(dead_code)]

use crate::bmo_abi::windowing::{BmoKey, BmoModifiers, BmoMouseButton};

// ─── Tipos de input ─────────────────────────────────────────────────

/// Categoría de evento de input.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoInputKind {
    None     = 0,
    KeyDown  = 1,
    KeyUp    = 2,
    MouseDown = 3,
    MouseUp   = 4,
    MouseMove = 5,
    MouseWheel = 6,
    GamepadButton = 7,
    GamepadAxis   = 8,
}

/// Estado del teclado (snapshot, no evento).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BmoKeyState {
    /// Bitmap de teclas presionadas.
    /// Cada bit representa un `BmoKey` (hasta 256 keys).
    pub keys: [u32; 8],
    pub modifiers: BmoModifiers,
}

impl BmoKeyState {
    /// `true` si la tecla está presionada.
    pub fn is_down(&self, k: BmoKey) -> bool {
        let k = k as u32;
        if k >= 256 { return false; }
        (self.keys[(k / 32) as usize] >> (k % 32)) & 1 != 0
    }

    /// Marca la tecla como presionada.
    pub fn set_down(&mut self, k: BmoKey) {
        let k = k as u32;
        if k >= 256 { return; }
        self.keys[(k / 32) as usize] |= 1 << (k % 32);
    }

    /// Marca la tecla como liberada.
    pub fn set_up(&mut self, k: BmoKey) {
        let k = k as u32;
        if k >= 256 { return; }
        self.keys[(k / 32) as usize] &= !(1 << (k % 32));
    }
}

/// Posición y botones del ratón (snapshot).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BmoMouseState {
    pub x: i32,
    pub y: i32,
    /// Bits de botones presionados (`BmoMouseButton`).
    pub buttons: u32,
    pub modifiers: BmoModifiers,
    pub wheel: i32,
}

impl BmoMouseState {
    pub fn is_down(&self, b: BmoMouseButton) -> bool {
        (self.buttons >> (b as u32)) & 1 != 0
    }
}

/// Botón de gamepad.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoGamepadButton {
    South = 0, // A / Cross
    East  = 1, // B / Circle
    West  = 2, // X / Square
    North = 3, // Y / Triangle
    L1    = 4,
    R1    = 5,
    L2    = 6,
    R2    = 7,
    Select = 8,
    Start  = 9,
    L3    = 10,
    R3    = 11,
    DpadUp    = 12,
    DpadDown  = 13,
    DpadLeft  = 14,
    DpadRight = 15,
}

/// Eje de gamepad (normalizado en `[-1.0, 1.0]` o `[0.0, 1.0]` para triggers).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoGamepadAxis {
    LeftX  = 0,
    LeftY  = 1,
    RightX = 2,
    RightY = 3,
    LeftTrigger  = 4,
    RightTrigger = 5,
}

/// Estado de gamepad.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BmoGamepadState {
    /// `true` si está conectado.
    pub connected: bool,
    pub _pad: [u8; 3],
    /// Botones presionados (bit por `BmoGamepadButton`).
    pub buttons: u32,
    /// Ejes. `LeftTrigger` / `RightTrigger` en `[0.0, 1.0]`, resto en `[-1.0, 1.0]`.
    pub axes: [f32; 6],
}

/// Evento genérico de input. Entregado por `bmo_input_poll_event`.
///
/// Tamaño: 32 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BmoInputEvent {
    pub kind: BmoInputKind,
    pub timestamp_ns: u64,
    /// Key code (si es keyboard), button (si es mouse), button (si es gamepad).
    pub code: u32,
    /// Mouse X (si es mouse) o axis index (si es gamepad axis).
    pub x: i32,
    /// Mouse Y (si es mouse) o axis value escalado a i32 (si es gamepad axis).
    pub y: i32,
    /// Modifiers activos al momento del evento.
    pub modifiers: BmoModifiers,
    pub _pad: u32,
}

impl BmoInputEvent {
    pub const SIZE: usize = 32;
}
