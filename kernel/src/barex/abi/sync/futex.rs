//! Futex BMO ABI — wait/wake en una palabra atómica de 32 bits.
//!
//! Reemplaza `WaitOnAddress` (Win32) y `futex(2)` (Linux). Modelo
//! deliberadamente más simple: una sola dirección, sin "bitsets".

#![allow(dead_code)]

use super::atomic::BmoAtomicU32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FutexOp {
    /// Si `*addr == expected`, suspende el thread actual.
    Wait        = 0,
    /// Despierta hasta `count` waiters de `addr`.
    WakeOne     = 1,
    /// Despierta a TODOS los waiters de `addr`.
    WakeAll     = 2,
    /// `requeue` — mueve waiters de `addr1` a `addr2` (raro pero útil para condvars).
    Requeue     = 3,
}

/// Futex tipado. Wrapper conveniente sobre `BmoAtomicU32`.
#[repr(transparent)]
#[derive(Debug)]
pub struct BmoFutex(pub BmoAtomicU32);

impl BmoFutex {
    pub const fn new(initial: u32) -> Self { Self(BmoAtomicU32::new(initial)) }

    /// Suspende si el valor actual es `expected` (carrera resuelta por el kernel).
    /// Devuelve `false` si el valor cambió antes de dormir.
    pub fn wait(&self, expected: u32, timeout_ns: u64) -> bool {
        let addr = &self.0 .0 as *const core::sync::atomic::AtomicU32 as *const u32;
        crate::syscall::futex_wait(addr, expected, timeout_ns)
    }

    pub fn wake_one(&self) -> u32 {
        let addr = &self.0 .0 as *const core::sync::atomic::AtomicU32 as *const u32;
        crate::syscall::futex_wake(addr, 1)
    }

    pub fn wake_all(&self) -> u32 {
        let addr = &self.0 .0 as *const core::sync::atomic::AtomicU32 as *const u32;
        crate::syscall::futex_wake(addr, u32::MAX)
    }
}
