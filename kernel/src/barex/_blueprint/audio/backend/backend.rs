//! Trait único que cualquier backend implementa. Reemplaza ASIO
//! (driver-by-driver) con una sola firma.

use crate::barex::{BxError, BxResult};
use super::super::format::{SampleFormat, ChannelLayout};

pub trait Backend {
    fn open(
        &mut self,
        sample_rate: u32,
        format: SampleFormat,
        channels: ChannelLayout,
        buffer_frames: u32,
    ) -> BxResult<()>;

    /// Empuja un bloque PCM al hardware. Idealmente cola DMA (no bloqueante).
    fn write_block(&mut self, samples: &[u8]) -> BxResult<()>;

    /// Lee del hardware (mic). Vacío si no hay capture.
    fn read_block(&mut self, _out: &mut [u8]) -> BxResult<usize> {
        Err(BxError::NotImplemented)
    }

    fn close(&mut self) -> BxResult<()>;

    fn name(&self) -> &'static str;
}
