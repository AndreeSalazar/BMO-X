//! `barex::audio` — `bx_audio`, audio nativo de FastOS sobre BMO ABI.
//!
//! Spec: `BareX_Audio_Spec.md`.
//!
//! ## Lo que **NO existe** aquí (eliminado por construcción)
//!
//! | Bloat eliminado            | Reemplazo BMO                          |
//! |----------------------------|----------------------------------------|
//! | WASAPI / IAudioClient COM  | `engine::BxAudioEngine`                |
//! | DirectSound / XAudio2      | `mixer::BxMixer` + `voice::BxVoice`    |
//! | ASIO (driver-by-driver)    | `backend::Backend` trait único         |
//! | CoreAudio HAL              | `route::Router`                        |
//! | ALSA + PulseAudio + JACK   | `backend::usb_ac2`, `backend::hdmi_gsp`|
//! | MMDevice / Endpoint COM    | `route::Endpoint`                      |
//! | KMixer / APO chain         | `effects::*`                           |
//! | `WAVEFORMATEX` zoo         | `format::Sample` + `format::Channels`  |
//! | Callbacks / event-driven   | `ring::` SQ/CQ io_uring-style          |
//!
//! ## Estructura modular (Sesión 11) — **no monolitos**
//!
//! ```
//!   audio/
//!   ├── mod.rs            ← este archivo (re-exports + versión)
//!   ├── capabilities.rs   ← AudioCapabilities bitflags
//!   ├── format/           ← SampleFormat, ChannelLayout, LatencyTier
//!   ├── engine/           ← BxAudioEngine + EngineMode
//!   ├── voice/            ← BxVoice (play/stop/volume/pitch)
//!   ├── mixer/            ← BxMixer (suma N voices en master)
//!   ├── codec/            ← PCM, Opus, Vorbis decoders
//!   ├── spatial/          ← BxSpatializer + ListenerPose (HRTF/Atmos)
//!   ├── effects/          ← EQ, reverb, compressor, limiter
//!   ├── route/            ← Endpoint routing (USB/HDMI/Realtek)
//!   ├── backend/          ← Backend trait + usb_ac2 + hdmi_gsp + realtek_hda
//!   └── ring/             ← SQ/CQ submit PCM low-latency
//! ```
//!
//! ## Latencias objetivo (headset USB Redragon)
//!
//! Ver [`format::LatencyTier`]. Modo `Realtime` = 32 frames @ 48 kHz ≈ 0.7 ms.

#![allow(dead_code)]

pub mod capabilities;
pub mod format;
pub mod engine;
pub mod voice;
pub mod mixer;
pub mod codec;
pub mod spatial;
pub mod effects;
pub mod route;
pub mod backend;
pub mod ring;

// ─── Re-exports planos ───────────────────────────────────────────────
pub use capabilities::AudioCapabilities;
pub use format::{SampleFormat, ChannelLayout, LatencyTier};
pub use engine::{BxAudioEngine, EngineMode, AudioBackend};
pub use voice::BxVoice;
pub use mixer::BxMixer;
pub use codec::{PcmDecoder, OpusDecoder, VorbisDecoder, CodecKind};
pub use spatial::{BxSpatializer, ListenerPose};
pub use effects::{BxEq, BxReverb, BxCompressor, BxLimiter, EffectKind};
pub use route::{Router, Endpoint, EndpointKind};
pub use backend::Backend;
pub use ring::{AudioSqe, AudioCqe, AudioSubmissionQueue, AudioCompletionQueue};

/// Versión ABI estable expuesta a apps Ring 3.
pub const BX_AUDIO_VERSION: (u8, u8, u8) = (1, 0, 0);

/// Formato por defecto para el headset Redragon (48 kHz / 16-bit / stereo).
pub const REDRAGON_DEFAULT_SR: u32 = 48_000;
