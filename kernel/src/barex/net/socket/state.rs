//! Estado de un socket (FSM TCP / UDP simplificada).

use crate::barex::abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Closed       = 0,
    Listening    = 1,
    SynSent      = 2,
    SynReceived  = 3,
    Established  = 4,
    FinWait      = 5,
    CloseWait    = 6,
    LastAck      = 7,
    TimeWait     = 8,
    /// Socket UDP — no hay handshake.
    Bound        = 9,
}

impl SocketState {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }

    #[inline(always)]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::Closed | Self::TimeWait)
    }
}
