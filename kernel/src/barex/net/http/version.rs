use crate::bmo_abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpVersion {
    Http2 = 2,
    Http3 = 3,
}

impl HttpVersion {
    pub const DEFAULT: Self = Self::Http3;
    #[inline(always)]
    pub const fn alpn(self) -> &'static [u8] {
        match self {
            Self::Http2 => b"h2",
            Self::Http3 => b"h3",
        }
    }
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }
}
