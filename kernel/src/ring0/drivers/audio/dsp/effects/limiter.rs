//! Brick-wall limiter con lookahead por ring buffer.

use crate::bmo_core::barex::BxResult;
use super::super::math::{dsp_abs, dsp_exp, dsp_max, dsp_min};

const SR: f32 = 48_000.0;
const MAX_LOOKAHEAD: usize = 256;

pub struct BxLimiter {
    pub ceiling_db: f32,
    pub lookahead_ms: f32,
    lookahead_samples: usize,
    read_pos: usize,
    write_pos: usize,
    gain_buf: [f32; MAX_LOOKAHEAD],
    pending_gain: f32,
    current_gain: f32,
}

impl BxLimiter {
    pub const SAFETY: Self = Self {
        ceiling_db: -0.3,
        lookahead_ms: 1.5,
        lookahead_samples: 0,
        read_pos: 0,
        write_pos: 0,
        gain_buf: [1.0; MAX_LOOKAHEAD],
        pending_gain: 1.0,
        current_gain: 1.0,
    };

    pub fn new(ceiling_db: f32, lookahead_ms: f32) -> Self {
        let lookahead_samples =
            ((lookahead_ms * SR / 1000.0) as u32).min(MAX_LOOKAHEAD as u32) as usize;
        let mut buf = [0.0f32; MAX_LOOKAHEAD];
        let mut i = 0;
        while i < MAX_LOOKAHEAD {
            buf[i] = 1.0;
            i += 1;
        }
        Self {
            ceiling_db,
            lookahead_ms,
            lookahead_samples,
            read_pos: 0,
            write_pos: 0,
            gain_buf: buf,
            pending_gain: 1.0,
            current_gain: 1.0,
        }
    }

    fn db_to_linear(db: f32) -> f32 {
        dsp_exp(db / 20.0 * core::f32::consts::LN_10)
    }

    pub fn process(&mut self, io: &mut [f32]) -> BxResult<()> {
        let ceiling = Self::db_to_linear(self.ceiling_db);
        let lookahead = self.lookahead_samples;
        let len = io.len();
        let mut i = 0;

        while i + 1 < len {
            let mut min_gain = 1.0f32;
            let mut j = 0;
            while j < lookahead && i + j + 1 < len {
                let peek_l = dsp_abs(io[i + j]);
                let peek_r = dsp_abs(io[i + j + 1]);
                let peek = dsp_max(peek_l, peek_r);
                if peek > ceiling {
                    let g = ceiling / peek;
                    min_gain = dsp_min(min_gain, g);
                }
                j += 2;
            }

            self.gain_buf[self.write_pos] = min_gain;
            self.write_pos += 1;
            if self.write_pos >= lookahead {
                self.write_pos = 0;
            }

            self.pending_gain = self.gain_buf[self.read_pos];
            self.read_pos += 1;
            if self.read_pos >= lookahead {
                self.read_pos = 0;
            }

            if self.pending_gain < self.current_gain {
                self.current_gain = self.pending_gain;
            } else {
                let attack = 1.0 - dsp_exp(-1.0 / (0.005 * SR));
                self.current_gain += (self.pending_gain - self.current_gain) * attack;
            }

            self.current_gain = dsp_min(self.current_gain, 1.0);
            self.current_gain = dsp_max(self.current_gain, 0.0);

            io[i] *= self.current_gain;
            io[i + 1] *= self.current_gain;
            i += 2;
        }
        Ok(())
    }
}
