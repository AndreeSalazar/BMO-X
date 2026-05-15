use crate::barex::abi::primitives::bx_u32;
use super::super::types::IpAddr;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DnsAnswer {
    pub addr: IpAddr,
    pub ttl_seconds: bx_u32,
}
