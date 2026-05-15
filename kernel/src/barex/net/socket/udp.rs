//! `BxUdpSocket` — UDP. Base para QUIC/HTTP3 y multicast LAN.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::handle::BmoHandle;
use super::super::types::Endpoint;
use super::state::SocketState;

pub struct BxUdpSocket {
    handle: BmoHandle,
    state: SocketState,
}

impl BxUdpSocket {
    pub fn open() -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    pub fn bind(&mut self, _local: Endpoint) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }

    pub fn send_to(&mut self, _data: &[u8], _peer: Endpoint) -> BxResult<u64> {
        Err(BxError::NotImplemented)
    }

    pub fn recv_from(&mut self, _buf: &mut [u8]) -> BxResult<(u64, Endpoint)> {
        Err(BxError::NotImplemented)
    }

    /// Multicast join. Requiere `NetCapabilities::MULTICAST`.
    pub fn join_multicast(&mut self, _group: Endpoint) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }

    #[inline(always)]
    pub const fn handle(&self) -> BmoHandle { self.handle }

    #[inline(always)]
    pub const fn state(&self) -> SocketState { self.state }
}
