//! `BxAudioEngine` — singleton per endpoint.
//!
//! Opens the first available backend in priority order:
//! 1. USB Audio Class 2.0 (e.g. Redragon headset)
//!
//! Mixes voices into a master bus and pushes PCM blocks to the backend.

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::handle::BmoHandle;
use crate::bmo_abi::handle::kind::HandleKind;
use super::super::format::{SampleFormat, ChannelLayout};
use super::super::voice::BxVoice;
use super::super::mixer::BxMixer;
use super::super::spatial::BxSpatializer;
use super::backend_kind::AudioBackend;
use super::mode::EngineMode;

/// Maximum voices the engine supports.
const MAX_VOICES: usize = 32;

pub struct BxAudioEngine {
    pub handle: BmoHandle,
    pub backend: AudioBackend,
    pub sample_rate: u32,
    pub channels: ChannelLayout,
    pub format: SampleFormat,
    pub buffer_frames: u32,
    mixer: BxMixer,
    voices: [Option<BxVoice>; MAX_VOICES],
    voice_count: usize,
    mix_buf: [f32; 4096], // Interleaved stereo scratch buffer
    active: bool,
}

impl BxAudioEngine {
    /// Open engine using USB Audio Class 2.0 backend.
    pub fn open(mode: EngineMode) -> BxResult<Self> {
        let (sr, fmt, ch, buf_frames) = match mode {
            EngineMode::ExclusiveOrShared { sample_rate, format, channels, buffer_frames }
            | EngineMode::Exclusive { sample_rate, format, channels, buffer_frames } => {
                (sample_rate, format, channels, buffer_frames)
            }
            EngineMode::Shared => {
                (48_000, SampleFormat::I16, ChannelLayout::Stereo, 128)
            }
        };

        // USB audio is initialized via attach() from USB enumeration.
        // We just use submit_pcm() to push audio.
        crate::drivers::serial::serial_write("[barex::audio] Engine opened (USB AC2 backend)\n");

        Ok(Self {
            handle: BmoHandle::new(HandleKind::AudioEngine, 0, 0),
            backend: AudioBackend::UsbAudioClass2,
            sample_rate: sr,
            channels: ch,
            format: fmt,
            buffer_frames: buf_frames,
            mixer: BxMixer::new_inner(),
            voices: Default::default(),
            voice_count: 0,
            mix_buf: [0.0; 4096],
            active: true,
        })
    }

    /// Create a voice from PCM data.
    pub fn create_voice(&mut self, pcm: &[i16]) -> BxResult<BxVoice> {
        if self.voice_count >= MAX_VOICES {
            return Err(BxError::OutOfMemory);
        }

        let voice = BxVoice::new_from_pcm(pcm, self.sample_rate);
        let idx = self.voice_count;
        self.voices[idx] = Some(voice.clone());
        self.voice_count += 1;
        Ok(voice)
    }

    /// Create a spatializer for 3D audio.
    pub fn create_spatializer(&self) -> BxResult<BxSpatializer> {
        BxSpatializer::new()
    }

    /// Mix all active voices and push one block to the backend.
    pub fn process_and_push(&mut self) -> BxResult<()> {
        if !self.active {
            return Err(BxError::NotInitialized);
        }

        let frames = self.buffer_frames as usize;
        let channels = 2usize; // stereo

        // Zero the mix buffer
        let total = frames * channels;
        if total > self.mix_buf.len() {
            return Err(BxError::InvalidArgument);
        }
        for s in self.mix_buf[..total].iter_mut() {
            *s = 0.0;
        }

        // Mix each active voice
        let mut active = 0u32;
        for i in 0..self.voice_count {
            if let Some(ref mut voice) = self.voices[i] {
                if voice.playing {
                    voice.mix_into(&mut self.mix_buf[..total], channels);
                    active += 1;
                }
            }
        }

        // Apply master volume
        let mv = self.mixer.master_volume;
        for s in self.mix_buf[..total].iter_mut() {
            *s *= mv;
        }

        // Convert f32 → i16 and push to USB audio
        let mut pcm_out = [0i16; 4096];
        for (i, &s) in self.mix_buf[..total].iter().enumerate() {
            let clamped = s.clamp(-1.0, 1.0);
            pcm_out[i] = (clamped * 32767.0) as i16;
        }

        // Push to USB audio driver
        // submit_pcm's endpoint param is unused (the driver tracks state globally)
        let dummy_ep = crate::drivers::usb::audio::AudioOutEndpoint {
            device: crate::drivers::usb::UsbDeviceId(0),
            ep_address: 0x01,
            max_packet_size: 480,
            interval_us: 1000,
            format: crate::drivers::usb::audio::StreamFormat {
                sample_rate: self.sample_rate,
                channels: 2,
                bits_per_sample: 16,
                frame_bytes: 4,
            },
        };
        let _ = crate::drivers::usb::audio::submit_pcm(&dummy_ep, &mut pcm_out[..total]);

        self.mixer.active_voices = active;
        Ok(())
    }

    /// Latency round-trip estimate in microseconds.
    pub fn latency_us(&self) -> u32 {
        let buf = (self.buffer_frames as u64 * 1_000_000 / self.sample_rate as u64) as u32;
        buf + 250 // + overhead xHCI + DMA + codec
    }

    /// Close engine and release resources.
    pub fn close(&mut self) -> BxResult<()> {
        self.active = false;
        self.voice_count = 0;
        Ok(())
    }
}
