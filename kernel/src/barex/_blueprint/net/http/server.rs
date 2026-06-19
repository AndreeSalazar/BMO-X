//! `Http3Server` — servidor HTTP/3.

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::handle::BmoHandle;
use super::super::types::Endpoint;

pub struct Http3Server {
    handle: BmoHandle,
}

impl Http3Server {
    pub fn bind(_local: Endpoint) -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    pub fn accept(&mut self) -> BxResult<BmoHandle> {
        Err(BxError::NotImplemented)
    }
}
