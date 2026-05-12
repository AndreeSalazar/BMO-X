//! `barex::audio` — `bx_audio`, audio nativo de FastOS.
//!
//! Spec: `BareX_Audio_Spec.md`. Objetivo < 1.5 ms round-trip en modo
//! exclusivo (Realtek ALC* / USB Audio Class 2/3 / HDMI Audio vía GSP).
//! Sin DirectSound, sin MMSystem, sin kmixer, sin "Audio Enhancements".

use crate::barex::{BxError, BxResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat { I16, I24, I32, F32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    Surround51,
    Surround71,
    Surround714,
}

#[derive(Debug, Clone, Copy)]
pub enum EngineMode {
    /// Exclusive si nadie más usa el endpoint, shared si hay otra app.
    ExclusiveOrShared {
        sample_rate: u32,
        format: SampleFormat,
        channels: ChannelLayout,
        buffer_frames: u32,
    },
    /// Forzar shared (mezclador del sistema).
    Shared,
}

#[derive(Debug, Clone, Copy)]
pub enum Latency {
    /// 32 frames @ 48 kHz ≈ 0.67 ms.
    Realtime,
    /// 64 frames ≈ 1.33 ms.
    LowLatency,
    /// 128 frames ≈ 2.67 ms (default).
    Balanced,
    /// 512 frames ≈ 10.67 ms.
    Power,
}

pub struct BxAudioEngine {
    _private: (),
}

pub struct BxVoice {
    _private: (),
}

pub struct BxSpatializer {
    _private: (),
}

impl BxAudioEngine {
    pub fn open(_mode: EngineMode) -> BxResult<Self> {
        // TODO: HDA controller driver + USB Audio Class.
        Err(BxError::NotImplemented)
    }
}
