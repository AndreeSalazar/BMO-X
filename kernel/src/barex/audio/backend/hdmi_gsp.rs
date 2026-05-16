//! Backend HDMI Audio vía GSP de la RTX 3060.
//! ⛔ Depende del bridge BMO/GSP en `drivers/gpu/fastgpu/` (usuario).

use crate::barex::{BxError, BxResult};
use super::backend::Backend;
use super::super::format::{SampleFormat, ChannelLayout};

pub struct HdmiGspBackend {
    pub display_index: u8,
}

impl Default for HdmiGspBackend {
    fn default() -> Self { Self { display_index: 0 } }
}

impl Backend for HdmiGspBackend {
    fn open(&mut self, _sr: u32, _f: SampleFormat, _c: ChannelLayout, _bf: u32) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
    fn write_block(&mut self, _samples: &[u8]) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
    fn close(&mut self) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
    fn name(&self) -> &'static str { "HDMI via GSP (GA106)" }
}
