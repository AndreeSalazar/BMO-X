//! Eventos discretos (alternativa al polling snapshot).

use crate::barex::abi::primitives::{bx_u8, bx_u32, bx_u64};
use crate::barex::abi::handle::BmoHandle;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEventKind {
    KeyDown          = 0,
    KeyUp            = 1,
    MouseMove        = 2,
    MouseButtonDown  = 3,
    MouseButtonUp    = 4,
    MouseWheel       = 5,
    GamepadButtonDown= 6,
    GamepadButtonUp  = 7,
    GamepadAxis      = 8,
    HeadsetButton    = 9,
    DevicePlugged    = 10,
    DeviceUnplugged  = 11,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub kind: bx_u8,
    pub _pad: [bx_u8; 7],
    pub device: BmoHandle,
    pub timestamp_ns: bx_u64,
    pub payload_lo: bx_u32,
    pub payload_hi: bx_u32,
}

impl InputEvent {
    pub const ZERO: Self = Self {
        kind: 0, _pad: [0; 7],
        device: BmoHandle(0),
        timestamp_ns: 0,
        payload_lo: 0, payload_hi: 0,
    };
}
