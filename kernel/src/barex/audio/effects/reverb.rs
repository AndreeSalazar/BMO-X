//! Reverb por convolución. Requiere `AudioCapabilities::HEAVY_DSP`.

use crate::barex::{BxError, BxResult};

pub struct BxReverb {
    pub room_size: f32, // 0..1
    pub damping: f32,   // 0..1
    pub wet: f32,       // 0..1
    pub dry: f32,       // 0..1
}

impl BxReverb {
    pub const SMALL_ROOM: Self = Self {
        room_size: 0.3, damping: 0.5, wet: 0.2, dry: 0.8,
    };

    pub fn process(&mut self, _io: &mut [f32]) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
