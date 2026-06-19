use super::super::format::{SampleFormat, ChannelLayout};

#[derive(Debug, Clone, Copy)]
pub enum EngineMode {
    /// Exclusive si nadie más usa el endpoint, sino shared.
    ExclusiveOrShared {
        sample_rate: u32,
        format: SampleFormat,
        channels: ChannelLayout,
        buffer_frames: u32,
    },
    /// Forzar shared (mezcla con otras apps vía `mixer::BxMixer`).
    Shared,
    /// Forzar exclusive (falla si endpoint ocupado).
    Exclusive {
        sample_rate: u32,
        format: SampleFormat,
        channels: ChannelLayout,
        buffer_frames: u32,
    },
}

impl EngineMode {
    pub const fn default_redragon() -> Self {
        Self::ExclusiveOrShared {
            sample_rate: super::super::REDRAGON_DEFAULT_SR,
            format: SampleFormat::I16,
            channels: ChannelLayout::Stereo,
            buffer_frames: 128,
        }
    }
}
