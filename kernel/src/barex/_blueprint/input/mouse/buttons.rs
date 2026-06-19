use crate::bmo_abi::primitives::bx_u8;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct MouseButtons: bx_u8 {
        const LEFT     = 1 << 0;
        const RIGHT    = 1 << 1;
        const MIDDLE   = 1 << 2;
        const BACK     = 1 << 3;
        const FORWARD  = 1 << 4;
        const EXTRA1   = 1 << 5;
        const EXTRA2   = 1 << 6;
        const EXTRA3   = 1 << 7;
    }
}
