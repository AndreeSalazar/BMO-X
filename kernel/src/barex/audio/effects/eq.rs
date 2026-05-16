//! EQ paramétrico 10 bandas. Implementación SIMD AVX2 pendiente.

use crate::barex::{BxError, BxResult};

pub struct BxEq {
    /// Ganancia por banda en dB.
    pub bands: [f32; 10],
}

impl BxEq {
    pub const FLAT: Self = Self { bands: [0.0; 10] };

    pub fn process(&mut self, _io: &mut [f32]) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
