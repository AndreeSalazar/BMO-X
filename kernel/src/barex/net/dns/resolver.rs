//! Resolver DoH/DoT con cache LRU. Sin `getaddrinfo`, sin /etc/hosts.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::handle::BmoHandle;
use super::answer::DnsAnswer;

pub struct DnsResolver {
    handle: BmoHandle,
}

impl DnsResolver {
    /// Crea resolver con upstream DoH (Cloudflare 1.1.1.1 / Google 8.8.8.8 / etc).
    pub fn new_doh(_upstream_url: &str) -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    pub fn new_dot(_upstream_host: &str) -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    pub fn resolve(&mut self, _hostname: &str, _out: &mut [DnsAnswer]) -> BxResult<u64> {
        Err(BxError::NotImplemented)
    }
}
