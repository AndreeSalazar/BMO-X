//! Vorbis decoder. Stub; deprecación recomendada en favor de Opus.

use crate::barex::{BxError, BxResult};

pub struct VorbisDecoder {
    pub sample_rate: u32,
    pub channels: u8,
}

impl VorbisDecoder {
    pub const fn new(sample_rate: u32, channels: u8) -> Self {
        Self { sample_rate, channels }
    }

    pub fn decode_packet(&mut self, _packet: &[u8], _out: &mut [f32]) -> BxResult<usize> {
        Err(BxError::NotImplemented)
    }
}
