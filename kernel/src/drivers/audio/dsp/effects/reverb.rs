//! Reverb basado en 8 lÃ­neas de delay paralelas con feedback.


use crate::barex::BxResult;
const SR: f32 = 48_000.0;
const MAX_DELAY: usize = 4096;
const NUM_DELAYS: usize = 8;

pub struct BxReverb {
    pub room_size: f32,
    pub damping: f32,
    pub wet: f32,
    pub dry: f32,
    delay_lines: [[f32; MAX_DELAY]; NUM_DELAYS],
    write_pos: [usize; NUM_DELAYS],
    delay_samples: [usize; NUM_DELAYS],
}

impl BxReverb {
    pub const SMALL_ROOM: Self = Self {
        room_size: 0.3,
        damping: 0.5,
        wet: 0.2,
        dry: 0.8,
        delay_lines: [[0.0; MAX_DELAY]; NUM_DELAYS],
        write_pos: [0; NUM_DELAYS],
        delay_samples: [0; NUM_DELAYS],
    };

    pub fn new(
        room_size: f32,
        damping: f32,
        wet: f32,
        dry: f32,
        delay_ms: &[f32; NUM_DELAYS],
    ) -> Self {
        let mut delay_samples = [0usize; NUM_DELAYS];
        let mut i = 0;
        while i < NUM_DELAYS {
            delay_samples[i] = (delay_ms[i] * SR / 1000.0) as usize;
            if delay_samples[i] >= MAX_DELAY {
                delay_samples[i] = MAX_DELAY - 1;
            }
            i += 1;
        }
        Self {
            room_size,
            damping,
            wet,
            dry,
            delay_lines: [[0.0; MAX_DELAY]; NUM_DELAYS],
            write_pos: [0; NUM_DELAYS],
            delay_samples,
        }
    }

    pub fn process(&mut self, io: &mut [f32]) -> BxResult<()> {
        let feedback = self.room_size * (1.0 - self.damping);
        let mut s = 0;
        while s + 1 < io.len() {
            let dry_left = io[s];
            let dry_right = io[s + 1];
            let dry_mono = (dry_left + dry_right) * 0.5;

            let mut wet_sum = 0.0f32;
            let mut i = 0;
            while i < NUM_DELAYS {
                let read_pos = if self.write_pos[i] >= self.delay_samples[i] {
                    self.write_pos[i] - self.delay_samples[i]
                } else {
                    MAX_DELAY - self.delay_samples[i] + self.write_pos[i]
                };
                let delayed = self.delay_lines[i][read_pos];
                self.delay_lines[i][self.write_pos[i]] = dry_mono + delayed * feedback;
                self.write_pos[i] += 1;
                if self.write_pos[i] >= MAX_DELAY {
                    self.write_pos[i] = 0;
                }
                wet_sum += delayed;
                i += 1;
            }

            wet_sum /= NUM_DELAYS as f32;
            io[s] = dry_left * self.dry + wet_sum * self.wet;
            io[s + 1] = dry_right * self.dry + wet_sum * self.wet;
            s += 2;
        }
        Ok(())
    }
}
