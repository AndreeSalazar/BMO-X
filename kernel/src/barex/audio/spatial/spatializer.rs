use crate::barex::BxResult;
use crate::barex::abi::handle::BmoHandle;
use crate::barex::audio::effects::dsp_math::{dsp_sqrt, dsp_acos};
use super::listener::ListenerPose;

pub struct BxSpatializer {
    pub handle: BmoHandle,
    pub listener: ListenerPose,
}

impl BxSpatializer {
    pub fn new() -> BxResult<Self> {
        Ok(Self {
            handle: BmoHandle(0),
            listener: ListenerPose::ORIGIN,
        })
    }

    pub fn set_listener(&mut self, p: ListenerPose) {
        self.listener = p;
    }

    pub fn process(&mut self, src: &[f32], dst: &mut [f32], voice_pos: [f32; 3]) -> BxResult<()> {
        let dx = voice_pos[0] - self.listener.pos[0];
        let dy = voice_pos[1] - self.listener.pos[1];
        let dz = voice_pos[2] - self.listener.pos[2];
        let distance = dsp_sqrt(dx * dx + dy * dy + dz * dz);

        let gain = 1.0 / (1.0 + distance * 0.1);

        // Horizontal angle between listener forward and voice direction
        let dot = dx * self.listener.forward[0]
            + dy * self.listener.forward[1]
            + dz * self.listener.forward[2];
        let horizontal_angle = if distance > 0.0001 {
            dsp_acos((dot / distance).clamp(-1.0, 1.0))
        } else {
            0.0
        };

        // Cross product Y component for left/right determination
        let cross_y = self.listener.forward[2] * dx - self.listener.forward[0] * dz;
        let pan = if distance > 0.0001 {
            (cross_y / distance).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        let left_gain = gain * (0.5 - pan * 0.5);
        let right_gain = gain * (0.5 + pan * 0.5);

        let src_frames = src.len();
        let dst_frames = dst.len() / 2;
        let frames = if src_frames < dst_frames { src_frames } else { dst_frames };

        let mut i = 0;
        let mut j = 0;
        while i < frames {
            let sample = src[i];
            dst[j] += sample * left_gain;
            dst[j + 1] += sample * right_gain;
            i += 1;
            j += 2;
        }

        let _ = horizontal_angle;
        Ok(())
    }
}
