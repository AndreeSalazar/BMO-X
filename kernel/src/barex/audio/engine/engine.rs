//! `BxAudioEngine` — singleton por endpoint.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::handle::BmoHandle;
use super::super::format::{SampleFormat, ChannelLayout};
use super::super::voice::BxVoice;
use super::super::spatial::BxSpatializer;
use super::backend_kind::AudioBackend;
use super::mode::EngineMode;

pub struct BxAudioEngine {
    pub handle: BmoHandle,
    pub backend: AudioBackend,
    pub sample_rate: u32,
    pub channels: ChannelLayout,
    pub format: SampleFormat,
    pub buffer_frames: u32,
}

impl BxAudioEngine {
    /// Abre engine: USB AC2 → HDMI genérico → Realtek HDA en ese orden.
    pub fn open(_mode: EngineMode) -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    pub fn create_voice(&self, _pcm: &[i16]) -> BxResult<BxVoice> {
        Err(BxError::NotImplemented)
    }

    pub fn create_spatializer(&self) -> BxResult<BxSpatializer> {
        Err(BxError::NotImplemented)
    }

    /// Latencia round-trip estimada en microsegundos.
    pub fn latency_us(&self) -> u32 {
        let buf = (self.buffer_frames as u64 * 1_000_000 / self.sample_rate as u64) as u32;
        // + overhead xHCI + DMA + codec headset
        buf + 250
    }

    pub fn close(self) -> BxResult<()> { Err(BxError::NotImplemented) }
}
