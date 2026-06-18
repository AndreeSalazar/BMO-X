use crate::bmo_abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadFamily {
    Xbox         = 0,
    PlayStation  = 1,
    Switch       = 2,
    Generic      = 3,
}

impl GamepadFamily {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }
}
