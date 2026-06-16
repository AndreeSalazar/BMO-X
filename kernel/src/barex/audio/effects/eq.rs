//! EQ paramétrico 10 bandas con filtros biquad.

use crate::barex::BxResult;
use super::dsp_math::{dsp_sin, dsp_cos, dsp_powf};

const SR: f32 = 48_000.0;

struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    const fn new() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process_sample(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

struct EqBand {
    freq: f32,
    gain_db: f32,
    q: f32,
    filter: Biquad,
}

impl EqBand {
    const fn new(freq: f32, gain_db: f32, q: f32) -> Self {
        Self {
            freq,
            gain_db,
            q,
            filter: Biquad::new(),
        }
    }

    fn compute_coefficients(&mut self) {
        let a = dsp_powf(10.0, self.gain_db / 40.0);
        let w0 = 2.0 * core::f32::consts::PI * self.freq / SR;
        let sin_w0 = dsp_sin(w0);
        let cos_w0 = dsp_cos(w0);
        let alpha = sin_w0 / (2.0 * self.q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        self.filter.b0 = b0 / a0;
        self.filter.b1 = b1 / a0;
        self.filter.b2 = b2 / a0;
        self.filter.a1 = a1 / a0;
        self.filter.a2 = a2 / a0;
    }
}

pub struct BxEq {
    pub bands: [f32; 10],
    filters: [EqBand; 10],
}

impl BxEq {
    pub const FLAT: Self = Self {
        bands: [0.0; 10],
        filters: [
            EqBand::new(31.0, 0.0, 0.707),
            EqBand::new(62.0, 0.0, 0.707),
            EqBand::new(125.0, 0.0, 0.707),
            EqBand::new(250.0, 0.0, 0.707),
            EqBand::new(500.0, 0.0, 0.707),
            EqBand::new(1_000.0, 0.0, 0.707),
            EqBand::new(2_000.0, 0.0, 0.707),
            EqBand::new(4_000.0, 0.0, 0.707),
            EqBand::new(8_000.0, 0.0, 0.707),
            EqBand::new(16_000.0, 0.0, 0.707),
        ],
    };

    pub fn new(
        freqs: &[f32; 10],
        gains_db: &[f32; 10],
        q_values: &[f32; 10],
    ) -> Self {
        let mut bands = [0.0f32; 10];
        let mut filters = [
            EqBand::new(31.0, 0.0, 0.707),
            EqBand::new(62.0, 0.0, 0.707),
            EqBand::new(125.0, 0.0, 0.707),
            EqBand::new(250.0, 0.0, 0.707),
            EqBand::new(500.0, 0.0, 0.707),
            EqBand::new(1_000.0, 0.0, 0.707),
            EqBand::new(2_000.0, 0.0, 0.707),
            EqBand::new(4_000.0, 0.0, 0.707),
            EqBand::new(8_000.0, 0.0, 0.707),
            EqBand::new(16_000.0, 0.0, 0.707),
        ];
        let mut i = 0;
        while i < 10 {
            bands[i] = gains_db[i];
            filters[i] = EqBand::new(freqs[i], gains_db[i], q_values[i]);
            filters[i].compute_coefficients();
            i += 1;
        }
        Self { bands, filters }
    }

    pub fn process(&mut self, io: &mut [f32]) -> BxResult<()> {
        for i in 0..10 {
            self.filters[i].gain_db = self.bands[i];
            self.filters[i].compute_coefficients();
        }

        let mut s = 0;
        while s + 1 < io.len() {
            let mut left = io[s];
            let mut right = io[s + 1];
            for i in 0..10 {
                left = self.filters[i].filter.process_sample(left);
                right = self.filters[i].filter.process_sample(right);
            }
            io[s] = left;
            io[s + 1] = right;
            s += 2;
        }
        Ok(())
    }
}
