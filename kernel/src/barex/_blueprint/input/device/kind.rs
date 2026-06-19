use crate::bmo_abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Keyboard           = 0,
    Mouse              = 1,
    HeadsetButtons     = 2,
    GamepadXbox        = 3,
    GamepadPlayStation = 4,
    GamepadSwitch      = 5,
    GamepadGeneric     = 6,
    Wheel              = 7,
    FlightStick        = 8,
    Hotas              = 9,
    VrController       = 10,
    Tablet             = 11,
    Touchscreen        = 12,
    HidRaw             = 13,
}

impl DeviceKind {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }

    #[inline(always)]
    pub const fn is_gamepad(self) -> bool {
        matches!(self, Self::GamepadXbox | Self::GamepadPlayStation
            | Self::GamepadSwitch | Self::GamepadGeneric)
    }
}
