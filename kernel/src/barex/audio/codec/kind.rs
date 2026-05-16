use crate::barex::abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecKind {
    /// Sin compresión.
    Pcm    = 0,
    /// Opus (RFC 6716) — voz/música baja latencia, default streaming.
    Opus   = 1,
    /// Vorbis — música general, deprecado lentamente por Opus.
    Vorbis = 2,
    /// FLAC — lossless. Recomendado para masters locales.
    Flac   = 3,
}

impl CodecKind {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }
    /// Default recomendado para nuevo contenido (Opus 96–128 kbps).
    pub const DEFAULT_LOSSY:    Self = Self::Opus;
    pub const DEFAULT_LOSSLESS: Self = Self::Flac;
}
