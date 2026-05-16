//! Brick-wall limiter (lookahead). Última etapa antes del DAC.

use crate::barex::{BxError, BxResult};

pub struct BxLimiter {
    pub ceiling_db: f32,
    pub lookahead_ms: f32,
}

impl BxLimiter {
    pub const SAFETY: Self = Self {
        ceiling_db: -0.3, lookahead_ms: 1.5,
    };

    pub fn process(&mut self, _io: &mut [f32]) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
