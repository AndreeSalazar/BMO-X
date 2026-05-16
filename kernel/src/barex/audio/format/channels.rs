//! Layout de canales. Reemplaza `KSAUDIO_CHANNEL_CONFIG` (Win32).

use crate::barex::abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelLayout {
    Mono        = 1,
    Stereo      = 2,
    Surround51  = 6,
    Surround71  = 8,
    /// Atmos 7.1.4 (12 canales).
    Surround714 = 12,
    /// Atmos 9.1.6 (16 canales) — cine doméstico high-end.
    Surround916 = 16,
}

impl ChannelLayout {
    #[inline(always)]
    pub const fn count(self) -> u8 { self as u8 }

    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }

    /// Bytes por frame dado un `SampleFormat`.
    #[inline(always)]
    pub const fn bytes_per_frame(self, bytes_per_sample: usize) -> usize {
        (self.count() as usize) * bytes_per_sample
    }
}
