//! CPU flags como identificadores fuertes (para `cuando cf { ... }`).

use crate::bmo_abi::primitives::bx_u8;
use super::super::lexer::TokenKind;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuFlag {
    Carry     = 0,
    Zero      = 1,
    Sign      = 2,
    Overflow  = 3,
    Parity    = 4,
    Direction = 5,
}

impl CpuFlag {
    #[inline(always)]
    pub const fn raw(self) -> bx_u8 { self as bx_u8 }

    /// Mapea token de flag → CpuFlag.
    pub const fn from_token(t: TokenKind) -> Option<Self> {
        Some(match t {
            TokenKind::FlagCf => Self::Carry,
            TokenKind::FlagZf => Self::Zero,
            TokenKind::FlagSf => Self::Sign,
            TokenKind::FlagOf => Self::Overflow,
            TokenKind::FlagPf => Self::Parity,
            TokenKind::FlagDf => Self::Direction,
            _ => return None,
        })
    }

    /// Opcode `Jcc` corto que toma el salto si el flag está SET.
    /// (Para `cuando flag { ... }` el emisor usa el opuesto + jmp back.)
    pub const fn jcc_short_opcode(self) -> u8 {
        match self {
            Self::Carry     => 0x72, // JC
            Self::Zero      => 0x74, // JZ
            Self::Sign      => 0x78, // JS
            Self::Overflow  => 0x70, // JO
            Self::Parity    => 0x7A, // JP
            Self::Direction => 0x00, // DF no tiene Jcc directo; el emisor maneja
        }
    }
}
