//! Opus decoder (RFC 6716). Stub; impl real será SIMD AVX2 sobre Zen 3.

use crate::barex::{BxError, BxResult};

pub struct OpusDecoder {
    pub sample_rate: u32,
    pub channels: u8,
}

impl OpusDecoder {
    pub const fn new(sample_rate: u32, channels: u8) -> Self {
        Self { sample_rate, channels }
    }

    pub fn decode_packet(&mut self, _packet: &[u8], _out: &mut [f32]) -> BxResult<usize> {
        Err(BxError::NotImplemented)
    }
}
