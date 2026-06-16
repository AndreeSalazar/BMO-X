use crate::barex::{BxError, BxResult};
use crate::barex::abi::handle::BmoHandle;
use crate::barex::abi::handle::kind::HandleKind;
use crate::barex::audio::effects::dsp_math::{dsp_cos, dsp_sin};

pub struct BxMixer {
    pub handle: BmoHandle,
    /// Master volume 0.0..1.0.
    pub master_volume: f32,
    /// Number of active voices mixed in the last frame.
    pub active_voices: u32,
    /// Pan law: 0.0 = linear, 1.0 = -3dB equal power.
    pub pan_power: f32,
}

impl BxMixer {
    pub fn new() -> BxResult<Self> {
        Ok(Self::new_inner())
    }

    pub(crate) fn new_inner() -> Self {
        Self {
            handle: BmoHandle::new(HandleKind::AudioEngine, 0, 0),
            master_volume: 1.0,
            active_voices: 0,
            pan_power: 0.707, // -3dB equal power pan law
        }
    }

    /// Process a block mixing all provided voice buffers into `out`.
    ///
    /// `voices` is a slice of (voice_pcm, volume, pan, pitch_shift).
    /// `out` is interleaved stereo f32, already zeroed by caller.
    /// `channels` must be 2 (stereo).
    pub fn process_block(
        &mut self,
        out: &mut [f32],
        voices: &[(&[f32], f32, f32, f32)], // (pcm, volume, pan, pitch)
        channels: usize,
    ) -> BxResult<()> {
        if channels != 2 {
            return Err(BxError::InvalidArgument);
        }

        let frames = out.len() / channels;
        let mut active = 0u32;

        for (pcm, volume, pan, _pitch) in voices {
            if *volume <= 0.001 {
                continue;
            }
            active += 1;

            // Equal-power pan law
            let angle = (*pan + 1.0) * 0.25 * core::f32::consts::PI; // 0..PI/2
            let left_gain = dsp_cos(angle) * *volume;
            let right_gain = dsp_sin(angle) * *volume;

            let src_frames = pcm.len() / 2;
            let count = frames.min(src_frames);

            for f in 0..count {
                let l = pcm[f * 2];
                let r = pcm[f * 2 + 1];
                out[f * 2] += l * left_gain;
                out[f * 2 + 1] += r * right_gain;
            }
        }

        self.active_voices = active;
        Ok(())
    }

    #[inline(always)]
    pub fn set_master_volume(&mut self, v: f32) {
        self.master_volume = v.clamp(0.0, 1.0);
    }

    #[inline(always)]
    pub fn set_pan_power(&mut self, p: f32) {
        self.pan_power = p.clamp(0.0, 1.0);
    }
}
