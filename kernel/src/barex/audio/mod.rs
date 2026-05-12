//! `barex::audio` — `bx_audio`, audio nativo de FastOS sobre BMO ABI.
//!
//! Spec: `BareX_Audio_Spec.md`.
//!
//! ## Backends en este equipo
//!
//! 1. **USB Audio Class 2.0** (Redragon headset) — vía `drivers::usb::audio_class`.
//! 2. **HDMI Audio** (RTX 3060) — vía `drivers::gpu::fastgpu` (cuando bridge listo).
//! 3. **Realtek HD Audio** (codec onboard) — pendiente, no urgente.
//!
//! ## Latencias objetivo (con headset USB Redragon)
//!
//! | Modo       | Buffer  | Round-trip |
//! |------------|---------|------------|
//! | Realtime   | 32 fr.  | ~0.7 ms    |
//! | LowLatency | 64 fr.  | ~1.3 ms    |
//! | Balanced   | 128 fr. | ~2.7 ms (default) |
//! | Power      | 512 fr. | ~10.7 ms   |

#![allow(dead_code)]

use crate::barex::{BxError, BxResult};
use crate::barex::abi::BmoHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat { I16, I24, I32, F32 }

impl SampleFormat {
    pub const fn bytes_per_sample(self) -> usize {
        match self { Self::I16 => 2, Self::I24 => 3, Self::I32 | Self::F32 => 4 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    Surround51,
    Surround71,
    Surround714,
}

impl ChannelLayout {
    pub const fn count(self) -> u8 {
        match self {
            Self::Mono => 1, Self::Stereo => 2,
            Self::Surround51 => 6, Self::Surround71 => 8, Self::Surround714 => 12,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EngineMode {
    /// Modo recomendado: exclusive si nadie más usa el endpoint, shared si hay otra app.
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
pub enum LatencyTier {
    /// 32 frames @ 48 kHz ≈ 0.67 ms.
    Realtime,
    LowLatency,
    Balanced,
    Power,
}

impl LatencyTier {
    pub const fn buffer_frames_at_48k(self) -> u32 {
        match self { Self::Realtime => 32, Self::LowLatency => 64, Self::Balanced => 128, Self::Power => 512 }
    }
}

/// Backend físico activo del engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBackend {
    UsbAudioClass2,
    HdmiViaGsp,
    RealtekHda,
    None,
}

// ─────────────────────────────────────────────────────────────────────
//   Objetos núcleo
// ─────────────────────────────────────────────────────────────────────

/// Engine: punto de entrada (singleton por endpoint).
pub struct BxAudioEngine {
    pub handle: BmoHandle,
    pub backend: AudioBackend,
    pub sample_rate: u32,
    pub channels: ChannelLayout,
    pub format: SampleFormat,
    pub buffer_frames: u32,
}

/// Voice: una fuente PCM/Vorbis/Opus que se mezcla en el master.
pub struct BxVoice {
    pub handle: BmoHandle,
    pub volume: f32,
    pub pitch: f32,
}

/// Spatializer: HRTF, surround, Atmos-like.
pub struct BxSpatializer {
    pub handle: BmoHandle,
}

#[derive(Debug, Clone, Copy)]
pub struct ListenerPose {
    pub pos: [f32; 3],
    pub forward: [f32; 3],
    pub up: [f32; 3],
}

// ─────────────────────────────────────────────────────────────────────
//   API pública
// ─────────────────────────────────────────────────────────────────────

impl BxAudioEngine {
    /// Abre el engine con el primer endpoint USB Audio detectado (Redragon en
    /// este equipo). Si no hay USB Audio, intenta HDMI vía GSP.
    pub fn open(_mode: EngineMode) -> BxResult<Self> {
        // TODO: 1) preguntar a `drivers::usb::audio_class` si hay device attached
        //       2) si no, fallback a `drivers::gpu::fastgpu` (HDMI)
        //       3) configurar isoch endpoint OUT con el formato pedido
        //       4) registrar handle BMO en la tabla del kernel
        Err(BxError::NotImplemented)
    }

    pub fn create_voice(&self, _pcm: &[i16]) -> BxResult<BxVoice> {
        Err(BxError::NotImplemented)
    }

    pub fn create_spatializer(&self) -> BxResult<BxSpatializer> {
        Err(BxError::NotImplemented)
    }

    /// Devuelve la latencia round-trip estimada en microsegundos.
    pub fn latency_us(&self) -> u32 {
        // Asumimos USB Audio Class 2.0 a 48 kHz HighSpeed (1 ms isoch interval).
        let buffer_us = (self.buffer_frames as u64 * 1_000_000 / self.sample_rate as u64) as u32;
        // + overhead de driver xHCI + DMA + codec del headset
        buffer_us + 250
    }
}

impl BxVoice {
    pub fn play(&self) -> BxResult<()> { Err(BxError::NotImplemented) }
    pub fn stop(&self) -> BxResult<()> { Err(BxError::NotImplemented) }
    pub fn set_volume(&mut self, v: f32) { self.volume = v.clamp(0.0, 1.0); }
}
