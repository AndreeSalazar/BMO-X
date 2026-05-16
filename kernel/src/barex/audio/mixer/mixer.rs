use crate::barex::{BxError, BxResult};
use crate::barex::abi::handle::BmoHandle;

pub struct BxMixer {
    pub handle: BmoHandle,
    /// Volumen master 0.0..1.0.
    pub master_volume: f32,
    /// Número de voces activas mezcladas en el último frame.
    pub active_voices: u32,
}

impl BxMixer {
    pub fn new() -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    /// Procesa un bloque de N frames mezclando todas las voces ancladas.
    pub fn process_block(&mut self, _out: &mut [f32]) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }

    #[inline(always)]
    pub fn set_master_volume(&mut self, v: f32) {
        self.master_volume = v.clamp(0.0, 1.0);
    }
}
