//! `TlsContext` — contexto TLS 1.3 (handshake state + keys).

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::handle::BmoHandle;
use super::cipher::TlsCipherSuite;

pub struct TlsContext {
    handle: BmoHandle,
    suite: TlsCipherSuite,
}

impl TlsContext {
    /// Cliente con SNI explícito (sin hostname → handshake falla).
    pub fn new_client(_server_name: &str) -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    /// Servidor con cert + key. Cert format: BMO-cert (no PEM/DER bloat).
    pub fn new_server(_cert: &[u8], _key: &[u8]) -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    /// Procesa bytes del peer; produce bytes a enviar + bytes de aplicación.
    pub fn process(&mut self, _input: &[u8], _output: &mut [u8]) -> BxResult<(u64, u64)> {
        Err(BxError::NotImplemented)
    }

    #[inline(always)]
    pub const fn handle(&self) -> BmoHandle { self.handle }

    #[inline(always)]
    pub const fn suite(&self) -> TlsCipherSuite { self.suite }
}
