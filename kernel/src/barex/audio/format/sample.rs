//! Formato de muestra PCM. Sin `WAVE_FORMAT_PCM=0x0001` legacy.

use crate::bmo_abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    I16  = 0,
    I24  = 1,
    I32  = 2,
    F32  = 3,
    /// f64 raramente — solo masterizado profesional.
    F64  = 4,
}

impl SampleFormat {
    #[inline(always)]
    pub const fn bytes_per_sample(self) -> usize {
        match self {
            Self::I16 => 2,
            Self::I24 => 3,
            Self::I32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }

    #[inline(always)]
    pub const fn bits_per_sample(self) -> u32 {
        match self {
            Self::I16 => 16, Self::I24 => 24,
            Self::I32 => 32, Self::F32 => 32, Self::F64 => 64,
        }
    }

    #[inline(always)]
    pub const fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }
}
