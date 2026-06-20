//! `BmoRange` — rango semi-abierto `[start, end)` empacado.
//!
//! Reemplaza el patrón C `(start, end)` o `(offset, size)` que vive
//! disperso en código kernel/userspace. Cabe en 16 bytes / 2 GPRs.

use crate::bmo_core::bmo_abi::primitives::{bx_u64, bx_usize};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoRange {
    pub start: bx_u64,
    pub end:   bx_u64,
}

impl BmoRange {
    pub const EMPTY: Self = Self { start: 0, end: 0 };

    #[inline(always)]
    pub const fn new(start: bx_u64, end: bx_u64) -> Self {
        Self { start, end }
    }

    /// Construye desde `(offset, size)` — patrón estilo `mmap`.
    #[inline(always)]
    pub const fn from_offset_size(offset: bx_u64, size: bx_u64) -> Self {
        Self { start: offset, end: offset.saturating_add(size) }
    }

    #[inline(always)]
    pub const fn len(&self) -> bx_u64 {
        self.end.saturating_sub(self.start)
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool { self.start >= self.end }

    #[inline(always)]
    pub const fn contains(&self, point: bx_u64) -> bool {
        point >= self.start && point < self.end
    }

    /// Verifica si dos rangos se solapan (útil para validar barriers de GPU).
    #[inline(always)]
    pub const fn overlaps(&self, other: &BmoRange) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Intersección de dos rangos. `None` si no se solapan.
    pub fn intersect(&self, other: &BmoRange) -> Option<BmoRange> {
        if !self.overlaps(other) { return None; }
        let s = if self.start > other.start { self.start } else { other.start };
        let e = if self.end < other.end { self.end } else { other.end };
        Some(BmoRange::new(s, e))
    }

    /// Versión `usize` para slicing.
    #[inline(always)]
    pub const fn as_usize(&self) -> (bx_usize, bx_usize) {
        (self.start as bx_usize, self.end as bx_usize)
    }
}
