//! PCM "decoder" — copy + format conversion.

use crate::barex::{BxError, BxResult};
use super::super::format::SampleFormat;

pub struct PcmDecoder {
    pub format: SampleFormat,
    pub channels: u8,
    pub sample_rate: u32,
}

impl PcmDecoder {
    pub const fn new(format: SampleFormat, channels: u8, sample_rate: u32) -> Self {
        Self { format, channels, sample_rate }
    }

    /// Convierte de `format` a F32 normalizado. Bloque por bloque.
    pub fn decode_to_f32(&self, _input: &[u8], _out: &mut [f32]) -> BxResult<usize> {
        Err(BxError::NotImplemented)
    }
}
