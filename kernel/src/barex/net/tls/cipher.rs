//! Suites TLS 1.3 permitidas. Las **únicas** 5 que reconoce el RFC 8446.

use crate::barex::abi::primitives::bx_u16;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsCipherSuite {
    Aes128GcmSha256       = 0x1301,
    Aes256GcmSha384       = 0x1302,
    ChaCha20Poly1305Sha256= 0x1303,
    Aes128CcmSha256       = 0x1304,
    Aes128Ccm8Sha256      = 0x1305,
}

impl TlsCipherSuite {
    #[inline(always)]
    pub const fn iana(self) -> bx_u16 { self as bx_u16 }

    /// Suite por defecto recomendada para FastOS (Zen 3 tiene AES-NI veloz).
    pub const DEFAULT: Self = Self::Aes256GcmSha384;
}
