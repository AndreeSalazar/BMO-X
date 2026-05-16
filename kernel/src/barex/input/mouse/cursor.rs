use crate::barex::abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMode {
    Visible   = 0,
    Hidden    = 1,
    /// Sin cursor visible, deltas raw — modo FPS.
    Captured  = 2,
    /// Cursor visible pero confinado al rect de la ventana.
    Confined  = 3,
}

impl CursorMode {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }
}
