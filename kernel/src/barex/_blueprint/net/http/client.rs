//! `Http3Client` — cliente HTTP/3 sobre QUIC.

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::handle::BmoHandle;

pub struct Http3Client {
    handle: BmoHandle,
}

impl Http3Client {
    pub fn new() -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    /// GET sencillo. Devuelve handle a respuesta (stream QUIC).
    pub fn get(&mut self, _url: &str) -> BxResult<BmoHandle> {
        Err(BxError::NotImplemented)
    }

    pub fn post(&mut self, _url: &str, _body: &[u8]) -> BxResult<BmoHandle> {
        Err(BxError::NotImplemented)
    }
}
