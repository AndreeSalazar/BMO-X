//! Notación CIDR (`192.168.1.0/24`). Reemplaza `IN_ADDR + netmask`.

use crate::bmo_abi::primitives::bx_u8;
use super::ip::IpAddr;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    pub addr: IpAddr,
    /// Bits de prefijo (0..=32 para IPv4, 0..=128 para IPv6).
    pub prefix: bx_u8,
}

impl Cidr {
    #[inline(always)]
    pub const fn new(addr: IpAddr, prefix: u8) -> Self {
        Self { addr, prefix: prefix as bx_u8 }
    }

    #[inline(always)]
    pub const fn max_bits(&self) -> u8 {
        match self.addr { IpAddr::V4(_) => 32, IpAddr::V6(_) => 128 }
    }

    #[inline(always)]
    pub const fn is_valid(&self) -> bool {
        self.prefix <= self.max_bits()
    }
}
