//! `net` — tipos de red del BMO ABI.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u8, bx_u16};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoIpv4Addr {
    pub octets: [bx_u8; 4],
}

impl BmoIpv4Addr {
    pub const fn new(a: bx_u8, b: bx_u8, c: bx_u8, d: bx_u8) -> Self {
        Self { octets: [a, b, c, d] }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoIpv6Addr {
    pub octets: [bx_u8; 16],
}

impl BmoIpv6Addr {
    pub const fn new(a: [bx_u8; 16]) -> Self {
        Self { octets: a }
    }
}

#[repr(C)]
pub union BmoAddrUnion {
    pub ipv4: BmoIpv4Addr,
    pub ipv6: BmoIpv6Addr,
    pub _raw: [bx_u8; 16],
}

impl Copy for BmoAddrUnion {}
impl Clone for BmoAddrUnion {
    fn clone(&self) -> Self { *self }
}

impl core::fmt::Debug for BmoAddrUnion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe { f.debug_struct("BmoAddrUnion").field("_raw", &self._raw).finish() }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BmoSocketAddr {
    pub family: bx_u8,
    pub _pad: bx_u8,
    pub port: bx_u16,
    pub addr: BmoAddrUnion,
}

impl core::fmt::Debug for BmoSocketAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BmoSocketAddr")
            .field("family", &self.family)
            .field("port", &self.port)
            .field("addr", &self.addr)
            .finish()
    }
}

impl BmoSocketAddr {
    pub const AF_INET: bx_u8 = 4;
    pub const AF_INET6: bx_u8 = 6;

    pub const fn ipv4(addr: BmoIpv4Addr, port: bx_u16) -> Self {
        Self { family: Self::AF_INET, _pad: 0, port, addr: BmoAddrUnion { ipv4: addr } }
    }

    pub const fn ipv6(addr: BmoIpv6Addr, port: bx_u16) -> Self {
        Self { family: Self::AF_INET6, _pad: 0, port, addr: BmoAddrUnion { ipv6: addr } }
    }

    pub const fn is_ipv4(&self) -> bool { self.family == Self::AF_INET }
    pub const fn is_ipv6(&self) -> bool { self.family == Self::AF_INET6 }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoProtocol {
    Tcp  = 0,
    Udp  = 1,
    Icmp = 2,
}
