//! `BmoInstant` — punto en el tiempo, monotónico, ns desde boot.
//!
//! Backend: TSC (Time Stamp Counter) del Zen 3, escalado a ns por el HPET o
//! invariant-TSC frequency leído al boot. Resolución típica: ~0.3 ns por tick.
//! Latencia de `now()`: ~7 ciclos (≈ 2 ns en el 5600X a 3.7 GHz base).
//!
//! ## Inicialización
//!
//! `BmoInstant::now()` solo retorna `ZERO` hasta que el kernel llama a
//! `init(timestamp, tsc_freq)`. Una vez inicializado, todos los `now()`
//! posteriores retornan valores correctos.

use crate::bmo_abi::primitives::bx_u64;
use crate::bmo_abi::values::time::duration::BmoDuration;
use crate::bmo_abi::fundamentals::sync::{BmoAtomicU64, MemOrder};

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

    /// Lee el TSC y lo escala a nanosegundos. Latencia ~7 ciclos.
    ///
    /// Antes de `init()`, retorna `ZERO`.
    #[inline]
    pub fn now() -> Self {
        let tsc = crate::cpu::rdtsc();
        let ns = tsc_to_ns(tsc);
        Self { ns_since_boot: ns }
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

// ─── Init state — tsc_freq, tsc_at_boot, fixed-point multiplier ──────
//
// Para evitar división entera en `now()` (cuesta ~20+ ciclos), precalculamos
// un multiplicador de punto fijo: ns_per_tick_num / ns_per_tick_den.
//
// `ns_per_tick = 1_000_000_000 / tsc_freq_hz` (en punto fijo, Q32.32).

static TSC_FREQ_HZ:    BmoAtomicU64 = BmoAtomicU64::new(0);
static TSC_AT_BOOT:    BmoAtomicU64 = BmoAtomicU64::new(0);
static NS_PER_TICK_XFP: BmoAtomicU64 = BmoAtomicU64::new(0);
static INITIALIZED:     BmoAtomicU64 = BmoAtomicU64::new(0);

const Q32_SHIFT: u32 = 32;

/// Inicializa el backend de tiempo. Llamar UNA vez durante el boot del
/// kernel, después de calibrar el TSC.
///
/// - `tsc_at_boot`: valor de TSC en el momento de init.
pub fn init(tsc_at_boot: u64, tsc_freq_hz: u64) {
    if tsc_freq_hz == 0 {
        return;
    }
    TSC_AT_BOOT.store(tsc_at_boot, MemOrder::Release);
    TSC_FREQ_HZ.store(tsc_freq_hz, MemOrder::Release);

    // xfp = (1_000_000_000 << 32) / tsc_freq_hz
    let xfp = ((1_000_000_000u128) << Q32_SHIFT) / (tsc_freq_hz as u128);
    NS_PER_TICK_XFP.store(xfp as u64, MemOrder::Release);
    INITIALIZED.store(1, MemOrder::Release);
}

/// Indica si el backend de tiempo está inicializado.
#[inline(always)]
pub fn is_initialized() -> bool {
    INITIALIZED.load(MemOrder::Acquire) != 0
}

/// Frecuencia del TSC en Hz. Retorna 0 si no inicializado.
#[inline(always)]
pub fn tsc_freq_hz() -> u64 {
    TSC_FREQ_HZ.load(MemOrder::Acquire)
}

/// Convierte un valor TSC a nanosegundos desde el boot.
#[inline(always)]
pub fn tsc_to_ns(tsc: u64) -> u64 {
    if !is_initialized() {
        return 0;
    }
    let tsc_at_boot = TSC_AT_BOOT.load(MemOrder::Relaxed);
    let xfp = NS_PER_TICK_XFP.load(MemOrder::Relaxed);
    let delta = tsc.wrapping_sub(tsc_at_boot);
    // ns = (delta * xfp) >> 32
    // Para evitar overflow 64-bit con delta muy grandes, dividimos primero.
    // delta_max = 2^64, xfp_max = 2^32, product = 2^96 → overflow.
    // Estrategia: si delta > 2^32, dividirlo por 2^16 primero, xfp por 2^16.
    let (d, x) = if delta > (1u64 << 32) {
        (delta >> 16, xfp >> 16)
    } else {
        (delta, xfp)
    };
    let ns = (d as u128 * x as u128) >> Q32_SHIFT;
    ns as u64
}

/// Convierte nanosegundos a valor TSC (aproximado, dado `tsc_freq_hz`).
#[inline]
pub fn ns_to_tsc(ns: u64) -> u64 {
    if !is_initialized() {
        return 0;
    }
    let tsc_at_boot = TSC_AT_BOOT.load(MemOrder::Relaxed);
    let freq = TSC_FREQ_HZ.load(MemOrder::Relaxed);
    let tsc = ((ns as u128 * freq as u128) / 1_000_000_000u128) as u64;
    tsc_at_boot.wrapping_add(tsc)
}

/// Backoff busy-wait usando TSC. Aproximación basada en freq calibrada.
#[inline]
pub fn sleep(d: BmoDuration) {
    let start = crate::cpu::rdtsc();
    let target = ns_to_tsc(d.ns);
    while crate::cpu::rdtsc().wrapping_sub(start) < target {
        core::hint::spin_loop();
    }
}
