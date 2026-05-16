use crate::barex::abi::primitives::bx_u8;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Modifiers: bx_u8 {
        const L_CTRL  = 1 << 0;
        const L_SHIFT = 1 << 1;
        const L_ALT   = 1 << 2;
        const L_GUI   = 1 << 3;
        const R_CTRL  = 1 << 4;
        const R_SHIFT = 1 << 5;
        const R_ALT   = 1 << 6;
        const R_GUI   = 1 << 7;
    }
}

impl Modifiers {
    #[inline(always)]
    pub const fn any_ctrl(&self) -> bool {
        self.bits() & (Self::L_CTRL.bits() | Self::R_CTRL.bits()) != 0
    }
    #[inline(always)]
    pub const fn any_shift(&self) -> bool {
        self.bits() & (Self::L_SHIFT.bits() | Self::R_SHIFT.bits()) != 0
    }
    #[inline(always)]
    pub const fn any_alt(&self) -> bool {
        self.bits() & (Self::L_ALT.bits() | Self::R_ALT.bits()) != 0
    }
    #[inline(always)]
    pub const fn any_gui(&self) -> bool {
        self.bits() & (Self::L_GUI.bits() | Self::R_GUI.bits()) != 0
    }
}
