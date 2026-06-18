//! `BxQuicStream` — stream individual dentro de un `BxQuicEndpoint`.

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::primitives::bx_u64;

/// Stream ID QUIC (62-bit, encajado en u64).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuicStreamId(pub bx_u64);

impl QuicStreamId {
    #[inline(always)]
    pub const fn is_client_initiated(&self) -> bool { (self.0 & 0x1) == 0 }
    #[inline(always)]
    pub const fn is_bidirectional(&self) -> bool { (self.0 & 0x2) == 0 }
}

pub struct BxQuicStream {
    pub id: QuicStreamId,
}

impl BxQuicStream {
    pub fn write(&mut self, _data: &[u8], _fin: bool) -> BxResult<u64> {
        Err(BxError::NotImplemented)
    }

    pub fn read(&mut self, _buf: &mut [u8]) -> BxResult<u64> {
        Err(BxError::NotImplemented)
    }

    pub fn reset(&mut self, _error_code: u64) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}
