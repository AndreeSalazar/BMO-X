//! Backend Realtek HD Audio (codec onboard). Baja prioridad.

use crate::barex::{BxError, BxResult};
use super::backend::Backend;
use super::super::format::{SampleFormat, ChannelLayout};

pub struct RealtekHdaBackend;

impl Default for RealtekHdaBackend {
    fn default() -> Self { Self }
}

impl Backend for RealtekHdaBackend {
    fn open(&mut self, _sr: u32, _f: SampleFormat, _c: ChannelLayout, _bf: u32) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
    fn write_block(&mut self, _samples: &[u8]) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
    fn close(&mut self) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
    fn name(&self) -> &'static str { "Realtek HD Audio" }
}
