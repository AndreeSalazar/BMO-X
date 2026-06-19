//! BareX Input — System-level input management.
//!
//! Integrates PS/2 keyboard + USB HID into the BareX API.

#![allow(dead_code)]

use crate::drivers::serial;
use super::super::super::BxError;

/// Maximum number of input devices.
const MAX_DEVICES: usize = 8;

/// Maximum events per poll cycle.
const MAX_EVENTS: usize = 64;

/// Input device descriptor.
#[derive(Debug, Clone, Copy)]
pub struct DeviceInfo {
    pub kind: DeviceKind,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
    Gamepad,
    Headset,
    Unknown,
}

/// Input event types.
#[derive(Debug, Clone, Copy)]
pub enum InputEventKind {
    KeyDown,
    KeyUp,
    MouseMove,
    MouseButton,
    MouseWheel,
    GamepadButton,
    GamepadAxis,
    HeadsetButton,
}

/// A single input event.
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub kind: InputEventKind,
    pub device_index: u8,
    pub data0: u32,
    pub data1: u32,
    pub data2: u32,
}

impl InputEvent {
    pub const NONE: Self = Self {
        kind: InputEventKind::KeyDown,
        device_index: 0,
        data0: 0, data1: 0, data2: 0,
    };
}

/// Keyboard state snapshot.
#[derive(Debug, Clone, Copy)]
pub struct KeyboardState {
    pub keys_down: [u8; 32], // Bitmap: 256 keys
    pub modifiers: u8,       // Ctrl/Shift/Alt/GUI bits
    pub last_key: u8,
    pub last_char: u8,
}

impl KeyboardState {
    pub const EMPTY: Self = Self {
        keys_down: [0; 32],
        modifiers: 0,
        last_key: 0,
        last_char: 0,
    };

    pub fn is_key_down(&self, scancode: u8) -> bool {
        let byte = (scancode / 8) as usize;
        let bit = scancode % 8;
        if byte < 32 {
            (self.keys_down[byte] & (1 << bit)) != 0
        } else {
            false
        }
    }

    pub fn set_key(&mut self, scancode: u8, down: bool) {
        let byte = (scancode / 8) as usize;
        let bit = scancode % 8;
        if byte < 32 {
            if down {
                self.keys_down[byte] |= 1 << bit;
            } else {
                self.keys_down[byte] &= !(1 << bit);
            }
        }
    }
}

/// Mouse state snapshot.
#[derive(Debug, Clone, Copy)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub delta_x: i32,
    pub delta_y: i32,
    pub buttons: u8,     // Bit 0=left, 1=right, 2=middle
    pub wheel: i8,
}

impl MouseState {
    pub const EMPTY: Self = Self {
        x: 0, y: 0, delta_x: 0, delta_y: 0, buttons: 0, wheel: 0,
    };

    pub fn is_left_down(&self) -> bool { self.buttons & 0x01 != 0 }
    pub fn is_right_down(&self) -> bool { self.buttons & 0x02 != 0 }
    pub fn is_middle_down(&self) -> bool { self.buttons & 0x04 != 0 }
}

/// Cursor mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CursorMode {
    Visible,
    Hidden,
    Captured,
}

/// The BareX Input System.
pub struct BxInputSystem {
    devices: [DeviceInfo; MAX_DEVICES],
    device_count: usize,
    keyboard: KeyboardState,
    mouse: MouseState,
    event_queue: [InputEvent; MAX_EVENTS],
    event_count: usize,
    cursor_mode: CursorMode,
    initialized: bool,
}

static mut INPUT_SYSTEM: Option<BxInputSystem> = None;

impl BxInputSystem {
    fn new() -> Self {
        Self {
            devices: [DeviceInfo {
                kind: DeviceKind::Unknown,
                vendor_id: 0,
                product_id: 0,
                name: "",
            }; MAX_DEVICES],
            device_count: 0,
            keyboard: KeyboardState::EMPTY,
            mouse: MouseState::EMPTY,
            event_queue: [InputEvent::NONE; MAX_EVENTS],
            event_count: 0,
            cursor_mode: CursorMode::Visible,
            initialized: true,
        }
    }
}

/// Get the global input system instance.
pub fn instance() -> Result<&'static mut BxInputSystem, BxError> {
    unsafe {
        if INPUT_SYSTEM.is_none() {
            INPUT_SYSTEM = Some(BxInputSystem::new());
        }
        Ok(INPUT_SYSTEM.as_mut().unwrap())
    }
}

/// Initialize the input system.
pub fn init() -> Result<(), BxError> {
    let sys = instance()?;
    sys.device_count = 0;

    // Register PS/2 keyboard
    sys.devices[sys.device_count] = DeviceInfo {
        kind: DeviceKind::Keyboard,
        vendor_id: 0,
        product_id: 0,
        name: "PS/2 Keyboard",
    };
    sys.device_count += 1;

    // Register PS/2 mouse (if present)
    sys.devices[sys.device_count] = DeviceInfo {
        kind: DeviceKind::Mouse,
        vendor_id: 0,
        product_id: 0,
        name: "PS/2 Mouse",
    };
    sys.device_count += 1;

    serial::serial_write("[barex::input] Initialized with ");
    serial_write_usize(sys.device_count);
    serial::serial_write(" devices\n");

    Ok(())
}

/// Poll for input events. Returns number of events.
pub fn poll_events() -> Result<usize, BxError> {
    let sys = instance()?;
    sys.event_count = 0;

    // TODO: Pull events from USB HID keyboard/mouse drivers
    // The USB HID driver (drivers::usb::hid) routes raw reports.
    // When keyboard reports arrive, translate to InputEvent:
    //   kind = KeyDown/KeyUp, data0 = scancode, data1 = pressed

    Ok(sys.event_count)
}

/// Get current keyboard state.
pub fn keyboard_state() -> Result<KeyboardState, BxError> {
    let sys = instance()?;
    Ok(sys.keyboard)
}

/// Get current mouse state.
pub fn mouse_state() -> Result<MouseState, BxError> {
    let sys = instance()?;
    Ok(sys.mouse)
}

/// Set cursor mode.
pub fn set_cursor_mode(mode: CursorMode) -> Result<(), BxError> {
    let sys = instance()?;
    sys.cursor_mode = mode;
    Ok(())
}

/// Get cursor mode.
pub fn cursor_mode() -> Result<CursorMode, BxError> {
    let sys = instance()?;
    Ok(sys.cursor_mode)
}

/// Enumerate connected input devices.
pub fn enumerate_devices() -> Result<&'static [DeviceInfo], BxError> {
    let sys = instance()?;
    Ok(&sys.devices[..sys.device_count])
}

/// Get device count.
pub fn device_count() -> Result<usize, BxError> {
    let sys = instance()?;
    Ok(sys.device_count)
}

fn serial_write_usize(val: usize) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    let mut v = val;
    if v == 0 { i -= 1; buf[i] = b'0'; }
    else { while v > 0 { i -= 1; buf[i] = b'0' + (v % 10) as u8; v /= 10; } }
    serial::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}
