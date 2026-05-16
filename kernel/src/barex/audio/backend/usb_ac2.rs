//! Backend USB Audio Class 2.0 (Redragon headset). Vive sobre
//! `crate::drivers::usb::audio_class`.

use crate::barex::{BxError, BxResult};
use super::backend::Backend;
use super::super::format::{SampleFormat, ChannelLayout};

pub struct UsbAc2Backend {
    pub interface_index: u8,
    pub sample_rate: u32,
}

impl Default for UsbAc2Backend {
    fn default() -> Self { Self { interface_index: 0, sample_rate: 48_000 } }
}

impl Backend for UsbAc2Backend {
    fn open(&mut self, _sr: u32, _f: SampleFormat, _c: ChannelLayout, _bf: u32) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
    fn write_block(&mut self, _samples: &[u8]) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
    fn close(&mut self) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
    fn name(&self) -> &'static str { "USB Audio Class 2.0" }
}
