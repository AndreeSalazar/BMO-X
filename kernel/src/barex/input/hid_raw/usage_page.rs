//! HID Usage Pages — registro IANA `usb.org/sites/default/files/hut1_4.pdf`.

use crate::barex::abi::primitives::bx_u16;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidUsagePage {
    Undefined        = 0x00,
    /// Mouse, joystick, gamepad, keyboard (físicos de escritorio).
    GenericDesktop   = 0x01,
    Simulation       = 0x02,
    Vr               = 0x03,
    Sport            = 0x04,
    Game             = 0x05,
    /// Teclado.
    Keyboard         = 0x07,
    Leds             = 0x08,
    Button           = 0x09,
    /// Consumer Page — volumen, mute, media keys (headset Redragon).
    Consumer         = 0x0C,
    Digitizer        = 0x0D,
    SensorPage       = 0x20,
}

impl HidUsagePage {
    #[inline(always)]
    pub const fn raw(self) -> bx_u16 { self as bx_u16 }
}
