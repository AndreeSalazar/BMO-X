//! `Endpoint` — par (IP, puerto). Reemplaza `sockaddr_storage` (128 B) con 24 B.

use super::ip::IpAddr;
use super::port::Port;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Endpoint {
    pub ip: IpAddr,
    pub port: Port,
}

impl Endpoint {
    #[inline(always)]
    pub const fn new(ip: IpAddr, port: Port) -> Self {
        Self { ip, port }
    }

    #[inline(always)]
    pub const fn is_loopback(&self) -> bool { self.ip.is_loopback() }
}
