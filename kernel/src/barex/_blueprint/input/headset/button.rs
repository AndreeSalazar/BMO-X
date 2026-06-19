use crate::bmo_abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadsetButton {
    VolumeUp   = 0,
    VolumeDown = 1,
    Mute       = 2,
    MicMute    = 3,
    PlayPause  = 4,
    NextTrack  = 5,
    PrevTrack  = 6,
}

impl HeadsetButton {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }
}
