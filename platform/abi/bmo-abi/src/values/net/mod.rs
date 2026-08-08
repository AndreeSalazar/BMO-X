//! `net` -- tipos de red del BMO ABI.
//!
//! Reemplaza `<netinet/in.h>`, `<sys/socket.h>`, y todo el caos de tipos
//! de red de POSIX/Win32 con representaciones limpias y FFI-safe.

use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u8};

// --- IPv4 ----------------------------------------------------------

/// Direccion IPv4. 4 bytes, network byte order.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BmoIpv4Addr {
    pub octets: [bx_u8; 4],
}
const _: () = assert!(core::mem::size_of::<BmoIpv4Addr>() == 4);

impl BmoIpv4Addr {
    pub const UNSPECIFIED: Self = Self {
        octets: [0, 0, 0, 0],
    };
    pub const LOOPBACK: Self = Self {
        octets: [127, 0, 0, 1],
    };
    pub const BROADCAST: Self = Self {
        octets: [255, 255, 255, 255],
    };

    pub const fn new(a: bx_u8, b: bx_u8, c: bx_u8, d: bx_u8) -> Self {
        Self {
            octets: [a, b, c, d],
        }
    }

    /// From a u32 in host byte order.
    pub const fn from_u32(v: bx_u32) -> Self {
        Self {
            octets: [
                (v >> 24) as bx_u8,
                (v >> 16) as bx_u8,
                (v >> 8) as bx_u8,
                v as bx_u8,
            ],
        }
    }

    pub fn as_u32(&self) -> bx_u32 {
        (self.octets[0] as bx_u32) << 24
            | (self.octets[1] as bx_u32) << 16
            | (self.octets[2] as bx_u32) << 8
            | self.octets[3] as bx_u32
    }

    pub fn is_loopback(&self) -> bool {
        self.octets[0] == 127
    }

    pub fn is_private(&self) -> bool {
        self.octets[0] == 10
            || (self.octets[0] == 172 && (16..=31).contains(&self.octets[1]))
            || (self.octets[0] == 192 && self.octets[1] == 168)
    }

    pub fn is_unspecified(&self) -> bool {
        *self == Self::UNSPECIFIED
    }
}

// --- IPv6 ----------------------------------------------------------

/// Direccion IPv6. 16 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoIpv6Addr {
    pub segments: [bx_u16; 8],
}
const _: () = assert!(core::mem::size_of::<BmoIpv6Addr>() == 16);

impl BmoIpv6Addr {
    pub const UNSPECIFIED: Self = Self { segments: [0; 8] };
    pub const LOOPBACK: Self = Self {
        segments: [0, 0, 0, 0, 0, 0, 0, 1],
    };

    pub const fn new(segments: [bx_u16; 8]) -> Self {
        Self { segments }
    }
}

// --- SocketAddr ----------------------------------------------------

/// Direccion de socket: IPv4 o IPv6 + puerto.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoSocketAddr {
    V4(BmoSocketAddrV4),
    V6(BmoSocketAddrV6),
}

/// Socket address IPv4.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoSocketAddrV4 {
    pub addr: BmoIpv4Addr,
    pub port: bx_u16,
}

/// Socket address IPv6.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoSocketAddrV6 {
    pub addr: BmoIpv6Addr,
    pub port: bx_u16,
    pub flowinfo: bx_u32,
    pub scope_id: bx_u32,
}

// --- Protocol ------------------------------------------------------

/// Raw protocol number.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoRawProtocol(pub bx_u8);

/// Protocolo de transporte.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoProtocol {
    Tcp,
    Udp,
    Raw(BmoRawProtocol),
}

// --- Constants -----------------------------------------------------

impl BmoIpv4Addr {
    pub const LOCALHOST: Self = Self::LOOPBACK;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_loopback() {
        let ip = BmoIpv4Addr::LOOPBACK;
        assert!(ip.is_loopback());
    }

    #[test]
    fn ipv4_private() {
        let ip = BmoIpv4Addr::new(10, 0, 0, 1);
        assert!(ip.is_private());
    }

    #[test]
    fn ipv4_u32_roundtrip() {
        let ip = BmoIpv4Addr::new(192, 168, 1, 1);
        assert_eq!(BmoIpv4Addr::from_u32(ip.as_u32()), ip);
    }
}
