//! `clock` -- tipos de reloj del BMO ABI.
//!
//! Define los IDs de reloj del sistema y operaciones asociadas.
//! Reemplaza `clock_gettime` / `CLOCK_MONOTONIC` / etc. de POSIX.

use crate::bmo_abi::values::time::{BmoDuration, BmoInstant};

/// Identificador de reloj del sistema.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoClockId {
    /// Monotonico, tiempo desde boot (equivalente a CLOCK_MONOTONIC).
    Monotonic = 0,
    /// Tiempo real de pared (equivalente a CLOCK_REALTIME).
    Realtime = 1,
    /// TSC directa (sin calibracion, solo para intervalos).
    Tsc = 2,
    /// Tiempo de CPU del proceso actual.
    ProcessCpu = 3,
    /// Tiempo de CPU del thread actual.
    ThreadCpu = 4,
}

impl BmoClockId {
    /// Devuelve el `BmoInstant` actual para este reloj.
    pub fn now(&self) -> BmoInstant {
        match self {
            BmoClockId::Monotonic => BmoInstant::now(),
            BmoClockId::Realtime => BmoInstant::now(), // same backend for now
            BmoClockId::Tsc => BmoInstant::now(),
            _ => BmoInstant::ZERO,
        }
    }

    /// Resolucion del reloj en nanosegundos.
    pub fn resolution(&self) -> BmoDuration {
        // Todas usan TSC -> ~1 ns con calibracion, ~1 us sin ella
        BmoDuration::from_ns(1)
    }
}

// --- Sleep ---------------------------------------------------------

/// Suspende el hilo actual por `duration`.
pub fn sleep(duration: BmoDuration) {
    let start = BmoClockId::Monotonic.now();
    loop {
        let elapsed = BmoClockId::Monotonic.now().duration_since(start);
        if elapsed >= duration {
            break;
        }
        core::hint::spin_loop();
    }
}

/// Suspende el hilo actual hasta un instante absoluto.
pub fn sleep_until(instant: BmoInstant) {
    let now = BmoClockId::Monotonic.now();
    if instant > now {
        sleep(instant.duration_since(now));
    }
}
