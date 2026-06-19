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

    pub fn decode_to_f32(&self, input: &[u8], out: &mut [f32]) -> BxResult<usize> {
        let bps = self.format.bytes_per_sample();
        if bps == 0 {
            return Err(BxError::InvalidArgument);
        }
        let ch = self.channels as usize;
        if ch == 0 {
            return Err(BxError::InvalidArgument);
        }

        // Each frame = ch channels * bps bytes. Output per frame = ch f32s.
        let bytes_per_frame = bps * ch;
        let max_frames = input.len() / bytes_per_frame;
        let out_f32_slots = out.len() / ch;
        let frames = if max_frames < out_f32_slots { max_frames } else { out_f32_slots };
        let mut in_off: usize = 0;
        let mut out_off: usize = 0;

        match self.format {
            SampleFormat::I16 => {
                for _ in 0..frames {
                    for _ in 0..ch {
                        let lo = input[in_off] as i16;
                        let hi = input[in_off + 1] as i16;
                        let sample = lo | (hi << 8);
                        out[out_off] = sample as f32 / 32768.0;
                        in_off += 2;
                        out_off += 1;
                    }
                }
            }
            SampleFormat::I24 => {
                for _ in 0..frames {
                    for _ in 0..ch {
                        let b0 = input[in_off] as i32;
                        let b1 = input[in_off + 1] as i32;
                        let b2 = input[in_off + 2] as i32;
                        let mut sample = b0 | (b1 << 8) | (b2 << 16);
                        // Sign-extend from 24-bit
                        if sample & 0x80_00_00 != 0 {
                            sample |= 0xFF_00_00_00u32 as i32;
                        }
                        out[out_off] = sample as f32 / 8_388_608.0;
                        in_off += 3;
                        out_off += 1;
                    }
                }
            }
            SampleFormat::I32 => {
                for _ in 0..frames {
                    for _ in 0..ch {
                        let sample = i32::from_le_bytes([
                            input[in_off],
                            input[in_off + 1],
                            input[in_off + 2],
                            input[in_off + 3],
                        ]);
                        out[out_off] = sample as f32 / 2_147_483_648.0;
                        in_off += 4;
                        out_off += 1;
                    }
                }
            }
            SampleFormat::F32 => {
                let count = frames * ch;
                let mut i = 0;
                while i < count {
                    let val = f32::from_le_bytes([
                        input[in_off],
                        input[in_off + 1],
                        input[in_off + 2],
                        input[in_off + 3],
                    ]);
                    out[out_off] = val;
                    in_off += 4;
                    out_off += 1;
                    i += 1;
                }
            }
            SampleFormat::F64 => {
                for _ in 0..frames {
                    for _ in 0..ch {
                        let bytes = [
                            input[in_off],
                            input[in_off + 1],
                            input[in_off + 2],
                            input[in_off + 3],
                            input[in_off + 4],
                            input[in_off + 5],
                            input[in_off + 6],
                            input[in_off + 7],
                        ];
                        let val = f64::from_le_bytes(bytes);
                        out[out_off] = val as f32;
                        in_off += 8;
                        out_off += 1;
                    }
                }
            }
        }

        Ok(frames * ch)
    }
}
