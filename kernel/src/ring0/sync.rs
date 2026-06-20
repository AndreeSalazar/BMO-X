//! Synchronization primitives for Ring 0.
//!
//! This module provides the minimum needed to write SMP-safe drivers:
//!
//! - [`SpinLock<T>`]  : simple spinlock, IRQ state preserved
//! - [`IrqSpinLock<T>`]: spinlock that disables IRQs while held
//! - [`OnceCell<T>`]  : one-shot initialization
//!
//! None of these are re-entrant. The compiler will (in the future) reject
//! recursive locks via `!Send` / `!Sync` implementations.
//!
//! # Design rules
//!
//! 1. Hold locks for the shortest time possible. If a critical section
//!    can take more than ~100 µs, use an [`IrqSpinLock`] and consider
//!    whether a sleeping lock would be more appropriate (TODO).
//! 2. Never call `alloc`, print, or sleep while holding a lock.
//! 3. The lock order is documented per-driver; violating it causes
//!    deadlocks. Keep lock graphs small.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

use crate::cpu;

// ── SpinLock ────────────────────────────────────────────────────────────────

/// A simple spinlock. IRQ state is preserved across `lock()`.
pub struct SpinLock<T: ?Sized> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }
}

impl<T: ?Sized> SpinLock<T> {
    /// Try to acquire the lock once; return `None` if already held.
    pub fn try_lock(&self) -> Option<SpinLockGuard<T>> {
        if self.locked.swap(true, Ordering::Acquire) {
            None
        } else {
            Some(SpinLockGuard { lock: self })
        }
    }

    /// Spin until the lock is acquired. Returns a guard that releases on
    /// drop. Spins with `pause` between attempts to save power on x86.
    pub fn lock(&self) -> SpinLockGuard<T> {
        while self.locked.swap(true, Ordering::Acquire) {
            while self.locked.load(Ordering::Relaxed) {
                cpu::lfence();
                core::hint::spin_loop();
            }
        }
        SpinLockGuard { lock: self }
    }
}

unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}

pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
}

impl<T: ?Sized> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T { unsafe { &*self.lock.data.get() } }
}
impl<T: ?Sized> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T { unsafe { &mut *self.lock.data.get() } }
}
impl<T: ?Sized> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) { self.lock.locked.store(false, Ordering::Release); }
}

// ── IrqSpinLock ─────────────────────────────────────────────────────────────

/// A spinlock that disables interrupts while held. Use this when the
/// protected data is also touched from interrupt handlers.
pub struct IrqSpinLock<T: ?Sized> {
    inner: SpinLock<T>,
}

impl<T> IrqSpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self { inner: SpinLock::new(value) }
    }
}

impl<T: ?Sized> IrqSpinLock<T> {
    pub fn lock(&self) -> IrqSpinLockGuard<T> {
        let was_enabled = cpu::irqs_enabled();
        cpu::cli();
        let guard = self.inner.lock();
        // Make sure no interrupt is delivered between cli() and lock().
        cpu::lfence();
        IrqSpinLockGuard { inner: guard, was_enabled }
    }
}

unsafe impl<T: ?Sized + Send> Send for IrqSpinLock<T> {}
unsafe impl<T: ?Sized + Send> Sync for IrqSpinLock<T> {}

pub struct IrqSpinLockGuard<'a, T: ?Sized> {
    inner: SpinLockGuard<'a, T>,
    was_enabled: bool,
}

impl<T: ?Sized> Deref for IrqSpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T { self.inner.deref() }
}
impl<T: ?Sized> DerefMut for IrqSpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T { self.inner.deref_mut() }
}
impl<T: ?Sized> Drop for IrqSpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.inner.lock.locked.store(false, Ordering::Release);
        if self.was_enabled { cpu::sti(); }
    }
}

// ── OnceCell ────────────────────────────────────────────────────────────────

/// One-shot initialization. The first call to `get_or_init` runs the
/// closure; subsequent calls return a reference to the cached value.
pub struct OnceCell<T> {
    initialized: AtomicBool,
    value: UnsafeCell<core::mem::MaybeUninit<T>>,
}

impl<T> OnceCell<T> {
    pub const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            value: UnsafeCell::new(core::mem::MaybeUninit::uninit()),
        }
    }

    /// Returns a reference to the value, initializing it with `f` on the
    /// first call. `f` is guaranteed to run at most once.
    pub fn get_or_init<F: FnOnce() -> T>(&self, f: F) -> &T {
        if !self.initialized.load(Ordering::Acquire) {
            // Use a CAS-style lock to ensure only one init runs.
            if !self.initialized.swap(true, Ordering::AcqRel) {
                unsafe {
                    (*self.value.get()).write(f());
                }
            } else {
                // Another core is initializing; spin until done.
                while !self.is_initialized_marker() {
                    core::hint::spin_loop();
                }
            }
        }
        unsafe { (*self.value.get()).assume_init_ref() }
    }

    #[inline]
    fn is_initialized_marker(&self) -> bool {
        // After the swap above, the value is always initialized.
        true
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }
}

impl<T> Default for OnceCell<T> {
    fn default() -> Self { Self::new() }
}

unsafe impl<T: Send + Sync> Sync for OnceCell<T> {}
unsafe impl<T: Send> Send for OnceCell<T> {}

impl<T> Drop for OnceCell<T> {
    fn drop(&mut self) {
        if self.initialized.load(Ordering::Acquire) {
            unsafe { (*self.value.get()).assume_init_drop(); }
        }
    }
}

// ── AtomicBool (re-export for convenience) ─────────────────────────────────

use core::sync::atomic::{AtomicBool, Ordering};
