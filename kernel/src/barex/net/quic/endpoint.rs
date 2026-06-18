//! `BxQuicEndpoint` — endpoint QUIC (cliente o servidor).

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::handle::BmoHandle;
use super::super::types::Endpoint;
use super::stream::BxQuicStream;

pub struct BxQuicEndpoint {
    handle: BmoHandle,
}

impl BxQuicEndpoint {
    /// Crea endpoint asociado a un `BxUdpSocket` ya bound.
    pub fn bind(_local: Endpoint) -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    /// 0-RTT si hay session ticket cacheado, 1-RTT en frío.
    pub fn connect(&mut self, _peer: Endpoint, _server_name: &str) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }

    pub fn open_stream(&mut self, _bidirectional: bool) -> BxResult<BxQuicStream> {
        Err(BxError::NotImplemented)
    }

    pub fn close(self, _error_code: u64) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }

    #[inline(always)]
    pub const fn handle(&self) -> BmoHandle { self.handle }
}
