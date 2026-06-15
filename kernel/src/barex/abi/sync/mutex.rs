//! Mutex BMO — futex-backed, lock-free en el caso no contendido.
//!
//! Reemplaza `pthread_mutex_t`, `CRITICAL_SECTION`, `std::sync::Mutex`.
//! 4 bytes vs los ~40 de pthread_mutex_t en glibc.

#![allow(dead_code)]

use super::atomic::MemOrder;
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
            // Try to transition LOCKED -> LOCKED_WAITERS (mark that waiters exist).
            let prev = self.state.0.compare_exchange(
                STATE_LOCKED,
                STATE_LOCKED_WAITERS,
                MemOrder::Acquire,
                MemOrder::Acquire,
            );
            match prev {
                Ok(_) => {
                    // Successfully marked waiters. Now sleep.
                    self.state.wait(STATE_LOCKED_WAITERS, u64::MAX);
                    // Spurious wakeup: loop back and retry.
                }
                Err(STATE_FREE) => {
                    // Lock became free. Try to grab it.
                    if self.state.0.compare_exchange(
                        STATE_FREE,
                        STATE_LOCKED,
                        MemOrder::Acquire,
                        MemOrder::Relaxed,
                    ).is_ok() {
                        return;
                    }
                }
                Err(STATE_LOCKED_WAITERS) => {
                    // Someone else already set waiters flag. Just sleep.
                    self.state.wait(STATE_LOCKED_WAITERS, u64::MAX);
                }
                Err(_) => unreachable!(),
            }
        }
    }

    pub fn unlock(&self) {
        // Try LOCKED -> FREE (no waiters case).
        if self.state.0.compare_exchange(
            STATE_LOCKED,
            STATE_FREE,
            MemOrder::Release,
            MemOrder::Relaxed,
        ).is_ok() {
            return;
        }
        // LOCKED_WAITERS -> FREE + wake one.
        self.state.0.store(STATE_FREE, MemOrder::Release);
        self.state.wake_one();
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
