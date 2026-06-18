use crate::barex::BxResult;
use crate::bmo_abi::handle::BmoHandle;
use crate::bmo_abi::handle::kind::HandleKind;
use crate::barex::audio::effects::dsp_math::{dsp_cos, dsp_sin};

/// Maximum PCM frames a single voice can hold (2 seconds at 48kHz).
const MAX_PCM_FRAMES: usize = 96_000;

pub struct BxVoice {
    pub handle: BmoHandle,
    pub volume: f32,
    pub pitch: f32,
    /// 0.0 = full left, 1.0 = full right.
    pub pan: f32,
    /// True if playing in loop.
    pub looping: bool,
    /// Whether the voice is currently playing.
    pub playing: bool,
    /// PCM sample data (interleaved stereo f32).
    pcm_data: [f32; MAX_PCM_FRAMES * 2],
    pcm_frames: usize,
    /// Current playback position in frames.
    position: usize,
    /// Original sample rate of the source.
    source_rate: u32,
}

impl BxVoice {
    /// Create a new voice from 16-bit stereo PCM data.
    pub fn new_from_pcm(pcm_i16: &[i16], sample_rate: u32) -> Self {
        let mut voice = Self {
            handle: BmoHandle::new(HandleKind::AudioVoice, 0, 0),
            volume: 1.0,
            pitch: 1.0,
            pan: 0.0,
            looping: false,
            playing: false,
            pcm_data: [0.0; MAX_PCM_FRAMES * 2],
            pcm_frames: 0,
            position: 0,
            source_rate: sample_rate,
        };

        // Convert i16 → f32
        let frames = pcm_i16.len() / 2;
        let copy = frames.min(MAX_PCM_FRAMES);
        for i in 0..copy * 2 {
            voice.pcm_data[i] = pcm_i16[i] as f32 / 32768.0;
        }
        voice.pcm_frames = copy;
        voice
    }

    /// Create a voice from raw f32 stereo interleaved data.
    pub fn new_from_f32(pcm_f32: &[f32], sample_rate: u32) -> Self {
        let mut voice = Self {
            handle: BmoHandle::new(HandleKind::AudioVoice, 0, 0),
            volume: 1.0,
            pitch: 1.0,
            pan: 0.0,
            looping: false,
            playing: false,
            pcm_data: [0.0; MAX_PCM_FRAMES * 2],
            pcm_frames: 0,
            position: 0,
            source_rate: sample_rate,
        };

        let frames = pcm_f32.len() / 2;
        let copy = frames.min(MAX_PCM_FRAMES);
        voice.pcm_data[..copy * 2].copy_from_slice(&pcm_f32[..copy * 2]);
        voice.pcm_frames = copy;
        voice
    }

    /// Mix this voice into an output buffer with volume and pan applied.
    /// `out` is interleaved stereo, `channels` must be 2.
    pub fn mix_into(&mut self, out: &mut [f32], channels: usize) {
        if !self.playing || self.pcm_frames == 0 || channels != 2 {
            return;
        }

        let vol = self.volume;
        let angle = (self.pan + 1.0) * 0.25 * core::f32::consts::PI;
        let left_gain = dsp_cos(angle) * vol;
        let right_gain = dsp_sin(angle) * vol;

        let out_frames = out.len() / channels;
        let mut f = 0usize;

        while f < out_frames {
            let src_idx = self.position * 2;
            let dst_idx = f * 2;

            if src_idx + 1 >= self.pcm_frames * 2 {
                // End of sample data
                if self.looping {
                    self.position = 0;
                    continue;
                } else {
                    self.playing = false;
                    break;
                }
            }

            out[dst_idx] += self.pcm_data[src_idx] * left_gain;
            out[dst_idx + 1] += self.pcm_data[src_idx + 1] * right_gain;

            // Advance source position (pitch shift via step size)
            let step = self.pitch;
            self.position = (self.position as f32 + step) as usize;
            f += 1;
        }
    }

    pub fn play(&mut self) -> BxResult<()> {
        self.position = 0;
        self.playing = true;
        Ok(())
    }

    pub fn stop(&mut self) -> BxResult<()> {
        self.playing = false;
        self.position = 0;
        Ok(())
    }

    pub fn pause(&mut self) -> BxResult<()> {
        self.playing = false;
        Ok(())
    }

    pub fn resume(&mut self) -> BxResult<()> {
        self.playing = true;
        Ok(())
    }

    #[inline(always)]
    pub fn set_volume(&mut self, v: f32) { self.volume = v.clamp(0.0, 1.0); }

    #[inline(always)]
    pub fn set_pitch(&mut self, p: f32) { self.pitch = p.clamp(0.25, 4.0); }

    #[inline(always)]
    pub fn set_pan(&mut self, p: f32) { self.pan = p.clamp(-1.0, 1.0); }

    /// Current playback position in frames.
    pub fn position(&self) -> usize { self.position }

    /// Total frames in the voice.
    pub fn total_frames(&self) -> usize { self.pcm_frames }

    /// Whether the voice is done playing.
    pub fn is_done(&self) -> bool { !self.playing && !self.looping }
}

impl Clone for BxVoice {
    fn clone(&self) -> Self {
        let mut v = Self {
            handle: BmoHandle::new(HandleKind::AudioVoice, 0, 0),
            volume: self.volume,
            pitch: self.pitch,
            pan: self.pan,
            looping: self.looping,
            playing: false, // cloned voice starts paused
            pcm_data: [0.0; MAX_PCM_FRAMES * 2],
            pcm_frames: self.pcm_frames,
            position: 0,
            source_rate: self.source_rate,
        };
        v.pcm_data[..self.pcm_frames * 2].copy_from_slice(&self.pcm_data[..self.pcm_frames * 2]);
        v
    }
}
