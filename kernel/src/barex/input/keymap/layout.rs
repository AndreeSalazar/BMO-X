use crate::barex::abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    UsQwerty       = 0,
    UsDvorak       = 1,
    UsColemak      = 2,
    EsQwerty       = 3,
    EsLatinAmerica = 4,
    UkQwerty       = 5,
    DeQwertz       = 6,
    FrAzerty       = 7,
    JpJis          = 8,
}

impl Layout {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }
    pub const DEFAULT: Self = Self::UsQwerty;
}
