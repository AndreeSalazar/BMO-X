use crate::barex::{BxError, BxResult};

pub struct BxCompressor {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
}

impl BxCompressor {
    pub const TRANSPARENT: Self = Self {
        threshold_db: -18.0, ratio: 2.0, attack_ms: 5.0, release_ms: 50.0,
    };

    pub fn process(&mut self, _io: &mut [f32]) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
