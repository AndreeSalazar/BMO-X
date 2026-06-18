//! MAC Ethernet (6 bytes). Reemplaza `BYTE[6]` y `ether_addr`.

use crate::bmo_abi::primitives::bx_u8;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddr(pub [bx_u8; 6]);

impl MacAddr {
    pub const ZERO:      Self = Self([0; 6]);
    pub const BROADCAST: Self = Self([0xFF; 6]);

    #[inline(always)]
    pub const fn is_multicast(&self) -> bool { (self.0[0] & 0x01) != 0 }

    #[inline(always)]
    pub const fn is_locally_administered(&self) -> bool { (self.0[0] & 0x02) != 0 }
}
