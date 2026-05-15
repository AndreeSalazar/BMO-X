//! Trait que cualquier driver NIC concreto implementa.

use crate::barex::{BxError, BxResult};
use super::super::types::MacAddr;
use super::caps::NicCapabilities;

pub trait NicDriver {
    /// MAC addr del dispositivo.
    fn mac(&self) -> MacAddr;

    /// Capabilities (offloads, MTU, multi-queue).
    fn capabilities(&self) -> NicCapabilities;

    /// Envía un frame Ethernet listo (incluye headers L2).
    fn tx_frame(&mut self, frame: &[u8]) -> BxResult<()>;

    /// Bloqueante recibir (apps preferirán polling vía `ring::`).
    fn rx_frame(&mut self, buf: &mut [u8]) -> BxResult<usize>;

    /// Stub default: no-op.
    fn link_up(&mut self) -> BxResult<()> { Err(BxError::NotImplemented) }
    fn link_down(&mut self) -> BxResult<()> { Err(BxError::NotImplemented) }
}
