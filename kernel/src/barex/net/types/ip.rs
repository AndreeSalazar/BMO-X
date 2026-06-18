//! `IpAddr` — IPv4/IPv6 unificado, `repr(C)` 18 bytes total (1 tag + 16 dato + pad).

use crate::bmo_abi::primitives::bx_u8;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IpV4 {
    pub octets: [bx_u8; 4],
}

impl IpV4 {
    pub const ZERO:      Self = Self { octets: [0, 0, 0, 0] };
    pub const LOOPBACK:  Self = Self { octets: [127, 0, 0, 1] };
    pub const BROADCAST: Self = Self { octets: [255, 255, 255, 255] };

    #[inline(always)]
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self { octets: [a, b, c, d] }
    }

    #[inline(always)]
    pub const fn is_loopback(&self) -> bool { self.octets[0] == 127 }

    #[inline(always)]
    pub const fn is_unspecified(&self) -> bool {
        self.octets[0] == 0 && self.octets[1] == 0
        && self.octets[2] == 0 && self.octets[3] == 0
    }

    #[inline(always)]
    pub const fn is_multicast(&self) -> bool {
        self.octets[0] >= 224 && self.octets[0] <= 239
    }

    #[inline(always)]
    pub const fn as_u32(&self) -> u32 {
        u32::from_be_bytes(self.octets)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IpV6 {
    pub octets: [bx_u8; 16],
}

impl IpV6 {
    pub const ZERO: Self = Self { octets: [0; 16] };
    pub const LOOPBACK: Self = {
        let mut o = [0u8; 16];
        o[15] = 1;
        Self { octets: o }
    };

    #[inline(always)]
    pub const fn new(octets: [u8; 16]) -> Self { Self { octets } }

    #[inline(always)]
    pub const fn is_loopback(&self) -> bool {
        let mut i = 0; let mut all_zero_first = true;
        while i < 15 { if self.octets[i] != 0 { all_zero_first = false; } i += 1; }
        all_zero_first && self.octets[15] == 1
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpAddr {
    V4(IpV4),
    V6(IpV6),
}

impl IpAddr {
    pub const LOOPBACK_V4: Self = Self::V4(IpV4::LOOPBACK);
    pub const LOOPBACK_V6: Self = Self::V6(IpV6::LOOPBACK);

    #[inline(always)]
    pub const fn is_loopback(&self) -> bool {
        match self {
            Self::V4(a) => a.is_loopback(),
            Self::V6(a) => a.is_loopback(),
        }
    }

    #[inline(always)]
    pub const fn is_v4(&self) -> bool { matches!(self, Self::V4(_)) }
    #[inline(always)]
    pub const fn is_v6(&self) -> bool { matches!(self, Self::V6(_)) }
}
