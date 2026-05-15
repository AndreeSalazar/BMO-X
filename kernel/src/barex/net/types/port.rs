//! Puerto TCP/UDP. Reemplaza `uint16_t htons(port)` con tipo fuerte.

use crate::barex::abi::primitives::bx_u16;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Port(pub bx_u16);

impl Port {
    pub const ANY:  Self = Self(0);
    pub const HTTP: Self = Self(80);
    pub const HTTPS:Self = Self(443);
    pub const DNS:  Self = Self(53);
    pub const DOH:  Self = Self(443);
    pub const DOT:  Self = Self(853);
    pub const QUIC: Self = Self(443);

    #[inline(always)]
    pub const fn new(p: u16) -> Self { Self(p) }

    #[inline(always)]
    pub const fn raw(self) -> u16 { self.0 }

    /// Puertos <1024 requieren `NetCapabilities::PRIVILEGED_PORTS`.
    #[inline(always)]
    pub const fn is_privileged(self) -> bool { self.0 < 1024 }
}
