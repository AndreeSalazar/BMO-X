use crate::barex::abi::primitives::bx_u8;

/// Backend físico activo del engine.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBackend {
    None            = 0,
    UsbAudioClass2  = 1,
    HdmiViaGsp      = 2,
    RealtekHda      = 3,
}

impl AudioBackend {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }

    /// True si el backend está disponible sin GSP (no bloquea por bridge).
    #[inline(always)]
    pub const fn is_independent(self) -> bool {
        matches!(self, Self::UsbAudioClass2 | Self::RealtekHda)
    }
}
