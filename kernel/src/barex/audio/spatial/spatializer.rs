use crate::barex::{BxError, BxResult};
use crate::barex::abi::handle::BmoHandle;
use super::listener::ListenerPose;

pub struct BxSpatializer {
    pub handle: BmoHandle,
    pub listener: ListenerPose,
}

impl BxSpatializer {
    pub fn new() -> BxResult<Self> { Err(BxError::NotImplemented) }

    pub fn set_listener(&mut self, p: ListenerPose) { self.listener = p; }

    /// Espacializa una voz mono en N canales según pose del listener.
    pub fn process(&mut self, _src: &[f32], _dst: &mut [f32], _voice_pos: [f32; 3]) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
