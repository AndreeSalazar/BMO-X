//! IRQ-saving spinlock for short Ring 0 critical sections.
//!
//! The guard saves RFLAGS and clears IF while held, then restores IF only
//! if it was set on entry. This makes the lock safe to take from both
//! interrupt and non-interrupt context on the BSP, and SMP-ready once
//! application processors come online (contention is real then, so critical
//! sections must stay short — no waits, no logging inside the lock).

use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock {
    locked: AtomicBool,
}

pub struct Guard<'a> {
    lock: &'a SpinLock,
    rflags: u64,
}

impl SpinLock {
    pub const fn new() -> Self {
        Self { locked: AtomicBool::new(false) }
    }

    pub fn lock(&self) -> Guard<'_> {
        let rflags: u64;
        unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) rflags); }
        while self.locked.swap(true, Ordering::Acquire) {
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        Guard { lock: self, rflags }
    }
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
        if self.rflags & (1 << 9) != 0 {
            unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
        }
    }
}
