//! `BmoDuration` — intervalo de tiempo en nanosegundos. Reemplaza
//! `timespec`, `timeval`, `LARGE_INTEGER` para QPC y todo el zoo de C.

use crate::bmo_core::bmo_abi::primitives::bx_u64;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BmoDuration {
    pub ns: bx_u64,
}

impl BmoDuration {
    pub const ZERO: Self = Self { ns: 0 };
    pub const NANOSECOND:  Self = Self { ns: 1 };
    pub const MICROSECOND: Self = Self { ns: 1_000 };
    pub const MILLISECOND: Self = Self { ns: 1_000_000 };
    pub const SECOND:      Self = Self { ns: 1_000_000_000 };
    pub const MINUTE:      Self = Self { ns: 60 * 1_000_000_000 };
    pub const HOUR:        Self = Self { ns: 3600 * 1_000_000_000 };

    #[inline(always)]
    pub const fn from_ns(ns: bx_u64) -> Self { Self { ns } }

    #[inline(always)]
    pub const fn from_us(us: bx_u64) -> Self { Self { ns: us.saturating_mul(1_000) } }

    #[inline(always)]
    pub const fn from_ms(ms: bx_u64) -> Self { Self { ns: ms.saturating_mul(1_000_000) } }

    #[inline(always)]
    pub const fn from_secs(s: bx_u64) -> Self { Self { ns: s.saturating_mul(1_000_000_000) } }

    #[inline(always)]
    pub const fn as_ns(self) -> bx_u64 { self.ns }

    #[inline(always)]
    pub const fn as_us(self) -> bx_u64 { self.ns / 1_000 }

    #[inline(always)]
    pub const fn as_ms(self) -> bx_u64 { self.ns / 1_000_000 }

    #[inline(always)]
    pub const fn as_secs(self) -> bx_u64 { self.ns / 1_000_000_000 }

    #[inline(always)]
    pub const fn add(self, other: Self) -> Self {
        Self { ns: self.ns.saturating_add(other.ns) }
    }

    #[inline(always)]
    pub const fn sub(self, other: Self) -> Self {
        Self { ns: self.ns.saturating_sub(other.ns) }
    }

    #[inline(always)]
    pub const fn mul_u32(self, factor: u32) -> Self {
        Self { ns: self.ns.saturating_mul(factor as u64) }
    }
}
