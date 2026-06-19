use crate::bmo_abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    Eq         = 0,
    Reverb     = 1,
    Compressor = 2,
    Limiter    = 3,
    HighPass   = 4,
    LowPass    = 5,
    Chorus     = 6,
    Delay      = 7,
}

impl EffectKind {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }
}
