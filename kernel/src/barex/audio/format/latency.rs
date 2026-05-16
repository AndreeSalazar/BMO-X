//! Tiers de latencia. La app pide un tier; el engine elige `buffer_frames`.

use crate::barex::abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyTier {
    /// 32 frames @ 48 kHz ≈ 0.67 ms — requiere `AudioCapabilities::REALTIME`.
    Realtime    = 0,
    /// 64 frames ≈ 1.33 ms.
    LowLatency  = 1,
    /// 128 frames ≈ 2.67 ms — default recomendado.
    Balanced    = 2,
    /// 512 frames ≈ 10.67 ms — laptop / battery saver.
    Power       = 3,
}

impl LatencyTier {
    #[inline(always)]
    pub const fn buffer_frames_at_48k(self) -> u32 {
        match self {
            Self::Realtime => 32,
            Self::LowLatency => 64,
            Self::Balanced => 128,
            Self::Power => 512,
        }
    }

    #[inline(always)]
    pub const fn buffer_frames(self, sample_rate: u32) -> u32 {
        (self.buffer_frames_at_48k() as u64 * sample_rate as u64 / 48_000) as u32
    }

    /// Latencia teórica del buffer en microsegundos.
    #[inline(always)]
    pub const fn buffer_us_at_48k(self) -> u32 {
        (self.buffer_frames_at_48k() as u64 * 1_000_000 / 48_000) as u32
    }

    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }
}
