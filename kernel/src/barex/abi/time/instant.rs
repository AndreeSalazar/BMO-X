//! `BmoInstant` — punto en el tiempo, monotónico, ns desde boot.
//!
//! Backend: TSC (Time Stamp Counter) del Zen 3, escalado a ns por el HPET o
//! invariant-TSC frequency leído al boot. Resolución típica: ~0.3 ns por tick.
//! Latencia de `now()`: ~7 ciclos (≈ 2 ns en el 5600X a 3.7 GHz base).

use crate::barex::abi::primitives::bx_u64;
use super::duration::BmoDuration;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BmoInstant {
    /// Nanosegundos desde el boot (no Unix epoch). Monotónico, jamás retrocede.
    pub ns_since_boot: bx_u64,
}

impl BmoInstant {
    pub const ZERO: Self = Self { ns_since_boot: 0 };

    #[inline(always)]
    pub const fn from_ns(ns: bx_u64) -> Self {
        Self { ns_since_boot: ns }
    }

    /// Lee el TSC y lo escala. **Implementación pendiente** — vacante hasta
    /// integrar `arch::x86_64::tsc`.
    #[inline]
    pub fn now() -> Self {
        // TODO: usar `_rdtsc()` + frecuencia invariante calibrada al boot.
        Self::ZERO
    }

    /// Calcula la diferencia con otro instante. Si `other > self`, devuelve 0.
    #[inline(always)]
    pub const fn duration_since(self, other: Self) -> BmoDuration {
        BmoDuration::from_ns(self.ns_since_boot.saturating_sub(other.ns_since_boot))
    }

    /// Tiempo transcurrido desde este instante hasta `now()`.
    #[inline]
    pub fn elapsed(self) -> BmoDuration {
        Self::now().duration_since(self)
    }

    #[inline(always)]
    pub const fn add(self, d: BmoDuration) -> Self {
        Self { ns_since_boot: self.ns_since_boot.saturating_add(d.ns) }
    }

    #[inline(always)]
    pub const fn sub(self, d: BmoDuration) -> Self {
        Self { ns_since_boot: self.ns_since_boot.saturating_sub(d.ns) }
    }
}
