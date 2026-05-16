use crate::barex::{BxError, BxResult};
use crate::barex::abi::handle::BmoHandle;

pub struct BxVoice {
    pub handle: BmoHandle,
    pub volume: f32,
    pub pitch: f32,
    /// 0.0 = pleno-izquierda, 1.0 = pleno-derecha.
    pub pan: f32,
    /// True si reproduce en loop.
    pub looping: bool,
}

impl BxVoice {
    pub fn play(&self) -> BxResult<()> { Err(BxError::NotImplemented) }
    pub fn stop(&self) -> BxResult<()> { Err(BxError::NotImplemented) }
    pub fn pause(&self) -> BxResult<()> { Err(BxError::NotImplemented) }
    pub fn resume(&self) -> BxResult<()> { Err(BxError::NotImplemented) }

    #[inline(always)]
    pub fn set_volume(&mut self, v: f32) { self.volume = v.clamp(0.0, 1.0); }

    #[inline(always)]
    pub fn set_pitch(&mut self, p: f32) { self.pitch = p.clamp(0.25, 4.0); }

    #[inline(always)]
    pub fn set_pan(&mut self, p: f32) { self.pan = p.clamp(-1.0, 1.0); }
}
