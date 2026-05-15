//! `BxTcpSocket` — TCP nativo. Usa SQ/CQ del módulo `ring::`.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::handle::BmoHandle;
use super::super::types::Endpoint;
use super::state::SocketState;

pub struct BxTcpSocket {
    handle: BmoHandle,
    state: SocketState,
}

impl BxTcpSocket {
    pub fn open() -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    pub fn connect(&mut self, _peer: Endpoint) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }

    pub fn listen(&mut self, _local: Endpoint, _backlog: u32) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }

    /// Envía datos. Async — encola en SQ y devuelve `BxResult<()>`. La
    /// completion llega por CQ. Cero callbacks, cero `WSAOVERLAPPED`.
    pub fn send(&mut self, _data: &[u8]) -> BxResult<u64> {
        Err(BxError::NotImplemented)
    }

    pub fn recv(&mut self, _buf: &mut [u8]) -> BxResult<u64> {
        Err(BxError::NotImplemented)
    }

    pub fn close(self) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }

    #[inline(always)]
    pub const fn handle(&self) -> BmoHandle { self.handle }

    #[inline(always)]
    pub const fn state(&self) -> SocketState { self.state }
}
