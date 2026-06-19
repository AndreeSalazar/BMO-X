//! Compresor de dinÃ¡mica con detector de pico suavizado.

use crate::barex::BxResult;
use super::super::math::{dsp_abs, dsp_exp, dsp_max};

const SR: f32 = 48_000.0;

fn db_to_linear(db: f32) -> f32 {
    dsp_exp(db / 20.0 * core::f32::consts::LN_10)
}

pub struct BxCompressor {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    envelope: f32,
}

impl BxCompressor {
    pub const TRANSPARENT: Self = Self {
        threshold_db: -18.0,
        ratio: 2.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        envelope: 0.0,
    };

    pub fn new(threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32) -> Self {
        Self {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            envelope: 0.0,
        }
    }

    pub fn process(&mut self, io: &mut [f32]) -> BxResult<()> {
        let threshold_linear = db_to_linear(self.threshold_db);
        let attack_coeff = dsp_exp(-1.0 / (self.attack_ms * SR / 1000.0));
        let release_coeff = dsp_exp(-1.0 / (self.release_ms * SR / 1000.0));

        let mut i = 0;
        while i + 1 < io.len() {
            let left = dsp_abs(io[i]);
            let right = dsp_abs(io[i + 1]);
            let peak = dsp_max(left, right);

            let coeff = if peak > self.envelope {
                attack_coeff
            } else {
                release_coeff
            };
            self.envelope = coeff * self.envelope + (1.0 - coeff) * peak;

            let gain = if self.envelope > threshold_linear {
                let compressed =
                    threshold_linear + (self.envelope - threshold_linear) / self.ratio;
                compressed / self.envelope
            } else {
                1.0
            };

            io[i] *= gain;
            io[i + 1] *= gain;
            i += 2;
        }
        Ok(())
    }
}
