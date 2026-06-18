use crate::bmo_abi::primitives::bx_u8;

/// Backend físico activo del engine.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBackend {
    None            = 0,
    UsbAudioClass2  = 1,
    HdmiFramebuffer = 2,
    RealtekHda      = 3,
}

impl AudioBackend {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }

    /// True si el backend no depende de firmware/driver GPU dedicado.
    #[inline(always)]
    pub const fn is_independent(self) -> bool {
        matches!(self, Self::UsbAudioClass2 | Self::HdmiFramebuffer | Self::RealtekHda)
    }
}
