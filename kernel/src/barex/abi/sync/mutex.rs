//! Mutex BMO — futex-backed, lock-free en el caso no contendido.
//!
//! Reemplaza `pthread_mutex_t`, `CRITICAL_SECTION`, `std::sync::Mutex`.
//! 4 bytes vs los ~40 de pthread_mutex_t en glibc.

#![allow(dead_code)]

use super::atomic::{BmoAtomicU32, MemOrder};
use super::futex::BmoFutex;

/// Estado del lock empacado en 32 bits:
///   - 0 = libre
///   - 1 = tomado, sin waiters
///   - 2 = tomado, hay ≥ 1 waiter
const STATE_FREE: u32 = 0;
const STATE_LOCKED: u32 = 1;
const STATE_LOCKED_WAITERS: u32 = 2;

#[repr(transparent)]
#[derive(Debug)]
pub struct BmoMutex {
    state: BmoFutex,
}

impl BmoMutex {
    pub const fn new() -> Self {
        Self { state: BmoFutex::new(STATE_FREE) }
    }

    /// Toma el lock. Si está libre, salida en ~5 ciclos (cmpxchg fast path).
    /// Si hay contención, se duerme vía futex.
    pub fn lock(&self) {
        // Fast path: tomar libre → bloqueado sin waiters.
        if self.state.0.compare_exchange(
            STATE_FREE,
            STATE_LOCKED,
            MemOrder::Acquire,
            MemOrder::Relaxed,
        ).is_ok() {
            return;
        }
        self.lock_slow();
    }

    fn lock_slow(&self) {
        loop {
            // Marcar que hay waiters.
            let prev = self.state.0.swap(STATE_LOCKED_WAITERS, MemOrder::Acquire);
            if prev == STATE_FREE { return; }
            // Dormir hasta que alguien haga unlock.
            self.state.wait(STATE_LOCKED_WAITERS, u64::MAX);
        }
    }

    pub fn unlock(&self) {
        let prev = self.state.0.swap(STATE_FREE, MemOrder::Release);
        if prev == STATE_LOCKED_WAITERS {
            self.state.wake_one();
        }
    }

    /// Try-lock no bloqueante.
    pub fn try_lock(&self) -> bool {
        self.state.0.compare_exchange(
            STATE_FREE,
            STATE_LOCKED,
            MemOrder::Acquire,
            MemOrder::Relaxed,
        ).is_ok()
    }
}

impl Default for BmoMutex {
    fn default() -> Self { Self::new() }
}
